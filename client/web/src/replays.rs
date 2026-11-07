use std::num::NonZeroU32;

use ogar_core::net::replay::{PlaybackError, ReelId, ReplayLibrary, Sight};
use wasm_bindgen::prelude::*;

use crate::js_err;
use crate::world::WorldHandle;

fn errors(list: Vec<PlaybackError>) -> Option<String> {
    (!list.is_empty()).then(|| {
        list.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

#[wasm_bindgen]
#[derive(Default)]
pub struct Replays {
    pub(crate) inner: ReplayLibrary,
}

impl Replays {
    fn info(&self, id: ReelId) -> String {
        let info = self
            .inner
            .reel_infos()
            .into_iter()
            .find(|r| r.id == id)
            .expect("stored reel");
        serde_json::to_string(&info).expect("reel info serializes")
    }
}

#[wasm_bindgen]
impl Replays {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Replays {
        Replays::default()
    }

    #[wasm_bindgen(js_name = setWindow)]
    pub fn set_window(&mut self, window_ms: Option<u32>) -> Result<(), JsValue> {
        let window_ms = window_ms
            .map(|ms| NonZeroU32::new(ms).ok_or_else(|| js_err("capture window must be positive")))
            .transpose()?;
        self.inner.set_window(window_ms);
        Ok(())
    }

    #[wasm_bindgen(js_name = onConnect)]
    pub fn on_connect(&mut self) {
        self.inner.on_connect();
    }

    #[wasm_bindgen(js_name = recordSight)]
    pub fn record_sight(&mut self, world: &WorldHandle, eye: &[f32], own_aim: &[f32], now: f64) {
        let [eye_x, eye_y, scale] = <[f32; 3]>::try_from(eye).expect("eye is x, y, scale");
        let [own_aim_x, own_aim_y] = <[f32; 2]>::try_from(own_aim).expect("aim is x, y");
        let (aim_x, aim_y, splitting, aim_player) = match world.inner.spectate_aim() {
            Some(aim) => (aim.x, aim.y, aim.splitting, aim.owner as u16),
            None => (own_aim_x, own_aim_y, false, 0),
        };
        self.inner.record_sight(Sight {
            at: now,
            eye_x,
            eye_y,
            scale,
            aim_x,
            aim_y,
            splitting,
            aim_player,
        });
    }

    #[wasm_bindgen(js_name = cameraAt)]
    pub fn camera_at(&self, id: ReelId, position: f64) -> Option<Vec<f64>> {
        self.inner.camera_at(id, position).map(|s| {
            vec![
                f64::from(s.eye_x),
                f64::from(s.eye_y),
                f64::from(s.scale),
                f64::from(s.aim_x),
                f64::from(s.aim_y),
                f64::from(u8::from(s.splitting)),
                f64::from(s.aim_player),
            ]
        })
    }

    #[wasm_bindgen(js_name = captureWindow)]
    pub fn capture_window(&mut self, now: f64) -> Option<String> {
        let id = self.inner.capture_window(now)?;
        Some(self.info(id))
    }

    #[wasm_bindgen(js_name = toggleRecording)]
    pub fn toggle_recording(&mut self) -> Option<String> {
        let id = self.inner.toggle_recording()?;
        Some(self.info(id))
    }

    #[wasm_bindgen(js_name = isRecording)]
    pub fn is_recording(&self) -> bool {
        self.inner.is_recording()
    }

    #[wasm_bindgen(js_name = importReel)]
    pub fn import_reel(&mut self, bytes: &[u8]) -> Result<String, JsValue> {
        let id = self.inner.import_reel(bytes).map_err(js_err)?;
        Ok(self.info(id))
    }

    #[wasm_bindgen(js_name = exportReel)]
    pub fn export_reel(&self, id: ReelId) -> Option<Vec<u8>> {
        self.inner.export_reel(id)
    }

    #[wasm_bindgen(js_name = exportTrimmedReel)]
    pub fn export_trimmed_reel(&self, id: ReelId, from: f64, to: f64) -> Result<Vec<u8>, JsValue> {
        self.inner.export_trimmed_reel(id, from, to).map_err(js_err)
    }

    #[wasm_bindgen(js_name = deleteReel)]
    pub fn delete_reel(&mut self, id: ReelId) {
        self.inner.delete_reel(id);
    }

    #[wasm_bindgen(js_name = clearLibrary)]
    pub fn clear_library(&mut self) {
        self.inner.clear_library();
    }

    pub fn seek(
        &mut self,
        id: ReelId,
        position: f64,
        world: &mut WorldHandle,
        now: f64,
    ) -> Option<String> {
        match self.inner.seek(id, position, &mut world.inner, now) {
            Ok(list) => errors(list),
            Err(e) => Some(e.to_string()),
        }
    }

    #[wasm_bindgen(js_name = stopPlayback)]
    pub fn stop_playback(&mut self, world: &mut WorldHandle) {
        self.inner.stop_playback(&mut world.inner);
    }

    #[wasm_bindgen(js_name = scoutAhead)]
    pub fn scout_ahead(&mut self, id: ReelId, step_ms: f64, now: f64) -> Result<bool, JsValue> {
        self.inner.scout_ahead(id, step_ms, now).map_err(js_err)
    }

    #[wasm_bindgen(js_name = takeHud)]
    pub fn take_hud(&mut self) -> Option<String> {
        self.inner
            .take_hud()
            .map(|h| serde_json::to_string(&h).expect("HUD serializes"))
    }

    #[wasm_bindgen(js_name = seekForExport)]
    pub fn seek_for_export(
        &mut self,
        id: ReelId,
        position: f64,
        world: &mut WorldHandle,
        now: f64,
    ) -> Option<String> {
        match self
            .inner
            .seek_for_export(id, position, &mut world.inner, now)
        {
            Ok(list) => errors(list),
            Err(e) => Some(e.to_string()),
        }
    }

    #[wasm_bindgen(js_name = endExport)]
    pub fn end_export(&mut self) {
        self.inner.end_export();
    }
}
