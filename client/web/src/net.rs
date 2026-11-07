use ogar_core::macros::Macro;
use ogar_core::net::{Identity, MouseInput, Session};
use wasm_bindgen::prelude::*;

use crate::replays::Replays;
use crate::world::WorldHandle;
use crate::{js_err, tab};

fn identity(json: &str) -> Result<Identity, JsValue> {
    serde_json::from_str(json).map_err(js_err)
}

#[wasm_bindgen]
#[derive(Default)]
pub struct Net {
    session: Session,
}

#[wasm_bindgen]
impl Net {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Net {
        Net::default()
    }

    #[wasm_bindgen(js_name = onOpen)]
    pub fn on_open(&mut self, tab_index: u8, identity_json: &str, now: f64) -> Result<(), JsValue> {
        self.session
            .on_open(tab(tab_index)?, identity(identity_json)?, now);
        Ok(())
    }

    #[wasm_bindgen(js_name = onClose)]
    pub fn on_close(&mut self, tab_index: u8, world: &mut WorldHandle) -> Result<(), JsValue> {
        self.session.on_close(tab(tab_index)?, &mut world.inner);
        Ok(())
    }

    #[wasm_bindgen(js_name = isConnected)]
    pub fn is_connected(&self, tab_index: u8) -> Result<bool, JsValue> {
        Ok(self.session.is_connected(tab(tab_index)?))
    }

    #[wasm_bindgen(js_name = setIdentity)]
    pub fn set_identity(&mut self, tab_index: u8, identity_json: &str) -> Result<(), JsValue> {
        self.session
            .set_identity(tab(tab_index)?, identity(identity_json)?);
        Ok(())
    }

    pub fn receive(
        &mut self,
        tab_index: u8,
        data: &[u8],
        world: &mut WorldHandle,
        replays: &mut Replays,
        now: f64,
    ) -> Result<Option<String>, JsValue> {
        let tab = tab(tab_index)?;
        if let Some(identity) = self.session.identity(tab) {
            replays.inner.note_identity(tab, identity);
        }
        replays
            .inner
            .record(tab, data, self.session.record_policy(tab, data), now);
        Ok(self
            .session
            .receive(tab, data, &mut world.inner, now)
            .err()
            .map(|e| match data.first() {
                Some(op) => format!("first byte {op}: {e}"),
                None => format!("empty frame: {e}"),
            }))
    }

    #[wasm_bindgen(js_name = takeOutgoing)]
    pub fn take_outgoing(&mut self, tab_index: u8) -> Result<Option<Vec<u8>>, JsValue> {
        Ok(self.session.take_outgoing(tab(tab_index)?))
    }

    #[wasm_bindgen(js_name = takeHud)]
    pub fn take_hud(&mut self) -> Option<String> {
        self.session
            .take_hud()
            .map(|h| serde_json::to_string(&h).expect("HUD serializes"))
    }

    pub fn aim(&mut self, x: f32, y: f32, now: f64) -> bool {
        self.session.aim(x, y, now)
    }

    pub fn join(&mut self, preferred: Option<u8>, now: f64) -> Result<(), JsValue> {
        self.session.join(preferred.map(tab).transpose()?, now);
        Ok(())
    }

    pub fn spectate(&mut self, now: f64) {
        self.session.spectate(now);
    }

    #[wasm_bindgen(js_name = setFreeRoam)]
    pub fn set_free_roam(&mut self, free: bool, now: f64) {
        self.session.set_free_roam(free, now);
    }

    #[wasm_bindgen(js_name = onHotkey)]
    pub fn on_hotkey(&mut self, name: &str, down: bool, now: f64) -> bool {
        self.session.on_hotkey(name, down, now)
    }

    #[wasm_bindgen(js_name = runMacro)]
    pub fn run_macro(&mut self, json: &str, now: f64) -> Result<(), JsValue> {
        let m = Macro::parse(json).map_err(js_err)?;
        self.session.run_macro(&m, now);
        Ok(())
    }

    #[wasm_bindgen(js_name = stopMacros)]
    pub fn stop_macros(&mut self, now: f64) {
        self.session.stop_macros(now);
    }

    #[wasm_bindgen(js_name = markDialing)]
    pub fn mark_dialing(&mut self, tab_index: u8) -> Result<(), JsValue> {
        self.session.mark_dialing(tab(tab_index)?);
        Ok(())
    }

    #[wasm_bindgen(js_name = sendChat)]
    pub fn send_chat(&mut self, text: &str, now: f64) {
        self.session.send_chat(text, now);
    }

    #[wasm_bindgen(js_name = onMouse)]
    pub fn on_mouse(
        &mut self,
        button: u8,
        down: bool,
        x: f32,
        y: f32,
        world: &mut WorldHandle,
        now: f64,
    ) {
        let input = MouseInput {
            button,
            down,
            at: (x, y),
        };
        self.session.on_mouse(input, &mut world.inner, now);
    }

    #[wasm_bindgen(js_name = updatePointer)]
    pub fn update_pointer(&mut self, aim_x: f32, aim_y: f32) {
        self.session.update_pointer((aim_x, aim_y));
    }

    #[wasm_bindgen(js_name = nextTickAt)]
    pub fn next_tick_at(&self) -> Option<f64> {
        self.session.next_tick_at()
    }

    pub fn tick(&mut self, now: f64) {
        self.session.tick(now);
    }

    #[wasm_bindgen(js_name = setTabsAsUnits)]
    pub fn set_tabs_as_units(&mut self, on: bool) {
        self.session.set_tabs_as_units(on);
    }
}
