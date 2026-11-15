#!/usr/bin/env node
// Compares the tick time of this Rust server with the last JavaScript release.
//
//   node bench/compare.mjs [--bots 100,300,600] [--warmup 15] [--duration 60] [--js-ref 2fa3f12]
//
// Both servers time the same work per tick (move, collide, mode tick, client
// updates, leaderboard) and publish it as `update_time` on the stats port.
// The script runs each server with only bots connected, polls that value once
// per second after a warm-up, and prints the median, p95 and memory use.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { get } from "node:http";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const work = join(root, "target", "bench");
const exe = process.platform === "win32" ? ".exe" : "";

const opts = { bots: "100,300,600", warmup: "15", duration: "60", "js-ref": "2fa3f12" };
for (let i = 2; i < process.argv.length; i += 2) {
    const key = process.argv[i].replace(/^--/, "");
    if (!(key in opts) || process.argv[i + 1] === undefined) {
        console.error(`unknown or incomplete option ${process.argv[i]}`);
        process.exit(2);
    }
    opts[key] = process.argv[i + 1];
}
const botCounts = opts.bots.split(",").map(Number);
const warmup = Number(opts.warmup);
const duration = Number(opts.duration);
const gamePort = 18080;
const statsPort = 18088;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const run = (cmd, args, cwd) =>
    execFileSync(cmd, args, { cwd, stdio: "inherit", shell: process.platform === "win32" });

function prepareJs() {
    const dir = join(work, `js-${opts["js-ref"]}`);
    if (!existsSync(join(dir, "node_modules", "ws"))) {
        rmSync(dir, { recursive: true, force: true });
        mkdirSync(dir, { recursive: true });
        const tar = join(work, "js.tar");
        execFileSync("git", ["archive", "--format=tar", "-o", tar, opts["js-ref"]], { cwd: root });
        // Relative paths: GNU tar reads "D:\..." as a remote host.
        execFileSync("tar", ["-xf", "js.tar", "-C", `js-${opts["js-ref"]}`], { cwd: work });
        rmSync(tar);
        run("npm", ["install", "--no-audit", "--no-fund", "--omit=dev"], dir);
    }
    return dir;
}

function configureJs(dir, bots) {
    const file = join(dir, "src", "config.js");
    if (!existsSync(file + ".orig")) writeFileSync(file + ".orig", readFileSync(file));
    let text = readFileSync(file + ".orig", "utf8");
    const set = { serverBots: bots, serverPort: gamePort, serverStatsPort: statsPort, serverStatsUpdate: 1, serverTracker: 0 };
    for (const [key, value] of Object.entries(set)) {
        const re = new RegExp(`("${key}":\\s*)[^,\\n]+`);
        if (!re.test(text)) throw new Error(`config.js has no ${key}`);
        text = text.replace(re, `$1${value}`);
    }
    writeFileSync(file, text);
}

function configureRust(bots) {
    const file = join(work, "rust.toml");
    writeFileSync(
        file,
        [
            `serverBots = ${bots}`,
            `serverPort = ${gamePort}`,
            `serverStatsPort = ${statsPort}`,
            `serverStatsUpdate = 1`,
            `serverTracker = 0`,
        ].join("\n") + "\n",
    );
    return file;
}

function fetchStats() {
    return new Promise((resolveStats) => {
        const req = get({ host: "127.0.0.1", port: statsPort, path: "/", timeout: 1000 }, (res) => {
            let body = "";
            res.on("data", (c) => (body += c));
            res.on("end", () => {
                try {
                    resolveStats(JSON.parse(body));
                } catch {
                    resolveStats(null);
                }
            });
        });
        req.on("error", () => resolveStats(null));
        req.on("timeout", () => req.destroy());
    });
}

function rssMiB(pid) {
    try {
        if (process.platform === "linux") {
            const m = readFileSync(`/proc/${pid}/status`, "utf8").match(/VmRSS:\s+(\d+)/);
            return m ? Number(m[1]) / 1024 : NaN;
        }
        if (process.platform === "win32") {
            const out = execFileSync("tasklist", ["/fo", "csv", "/nh", "/fi", `PID eq ${pid}`], { encoding: "utf8" });
            const kb = out.split('","')[4]?.replace(/[^\d]/g, "");
            return kb ? Number(kb) / 1024 : NaN;
        }
        const out = execFileSync("ps", ["-o", "rss=", "-p", String(pid)], { encoding: "utf8" });
        return Number(out.trim()) / 1024;
    } catch {
        return NaN;
    }
}

function quantile(sorted, q) {
    const i = (sorted.length - 1) * q;
    const lo = Math.floor(i);
    return sorted[lo] + (sorted[Math.ceil(i)] - sorted[lo]) * (i - lo);
}

async function measure(name, cmd, args, cwd) {
    const child = spawn(cmd, args, { cwd, stdio: ["pipe", "ignore", "inherit"] });
    let exited = false;
    child.on("exit", () => (exited = true));
    try {
        const ready = Date.now() + 30_000;
        while (!(await fetchStats())) {
            if (exited || Date.now() > ready) throw new Error(`${name} did not open the stats port`);
            await sleep(250);
        }
        await sleep(warmup * 1000);
        const times = [];
        const rss = [];
        let bots = 0;
        for (let s = 0; s < duration; s++) {
            await sleep(1000);
            if (exited) throw new Error(`${name} exited during the run`);
            const stats = await fetchStats();
            if (!stats) continue;
            times.push(Number(stats.update_time));
            bots = Number(stats.bots);
            rss.push(rssMiB(child.pid));
        }
        times.sort((a, b) => a - b);
        const mem = rss.filter(Number.isFinite);
        return {
            name,
            bots,
            mean: times.reduce((a, b) => a + b, 0) / times.length,
            median: quantile(times, 0.5),
            p95: quantile(times, 0.95),
            rss: mem.length ? Math.max(...mem) : NaN,
        };
    } finally {
        child.kill();
        await new Promise((r) => (exited ? r() : child.once("exit", r)));
        await sleep(500);
    }
}

console.error("building the Rust server...");
run("cargo", ["build", "--release", "--quiet"], root);
mkdirSync(work, { recursive: true });
console.error(`preparing the JavaScript server at ${opts["js-ref"]}...`);
const jsDir = prepareJs();
const rustBin = join(root, "target", "release", `multiogarii${exe}`);

const rows = [];
for (const bots of botCounts) {
    console.error(`\n${bots} bots: JavaScript (${warmup}s warm-up, ${duration}s run)`);
    configureJs(jsDir, bots);
    const js = await measure("JavaScript", process.execPath, ["index.js"], join(jsDir, "src"));
    console.error(`${bots} bots: Rust`);
    const rs = await measure("Rust", rustBin, [configureRust(bots)], root);
    rows.push({ bots, js, rs });
}

const f = (n, d = 3) => (Number.isFinite(n) ? n.toFixed(d) : "-");
console.log(`\nnode ${process.version}, ${process.platform}-${process.arch}, ${duration} samples per run\n`);
console.log("| Bots | Server | Bots seen | Median tick (ms) | p95 tick (ms) | Mean tick (ms) | Peak RSS (MiB) |");
console.log("|---:|---|---:|---:|---:|---:|---:|");
for (const { bots, js, rs } of rows) {
    for (const r of [js, rs]) {
        console.log(`| ${bots} | ${r.name} | ${r.bots} | ${f(r.median)} | ${f(r.p95)} | ${f(r.mean)} | ${f(r.rss, 0)} |`);
    }
}
console.log("\n| Bots | Speed-up (median) | Speed-up (p95) |");
console.log("|---:|---:|---:|");
for (const { bots, js, rs } of rows) {
    console.log(`| ${bots} | ${f(js.median / rs.median, 1)}x | ${f(js.p95 / rs.p95, 1)}x |`);
}
