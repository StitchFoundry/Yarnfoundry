//! YarnFoundry in the browser: the same egui front end as the desktop app, drawn on a canvas. Nothing leaves the page:
//! pictures are read locally and prints and projects are saved as downloads. On other targets this crate is empty.

#[cfg(target_arch = "wasm32")]
mod page {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use yarnfoundry_ui_egui::YarnFoundryApp;

    /// A picture the user chose, waiting for the next frame to pick it up.
    type Inbox = Rc<RefCell<Option<Vec<u8>>>>;

    struct Host {
        app: YarnFoundryApp,
        inbox: Inbox,
    }

    impl eframe::App for Host {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            if let Some(bytes) = self.inbox.borrow_mut().take() {
                self.app.use_picture_bytes(&bytes);
            }
            self.app.ui(ui);
            let ctx = ui.ctx().clone();
            if self.app.take_load_picture().is_some() {
                if let Err(e) = choose_picture(self.inbox.clone(), ctx) {
                    self.app.set_message(format!("Could not open the file chooser: {e:?}"));
                }
            }
            if self.app.take_save_project() {
                let message = match self.app.chart().map(save_project) {
                    Some(Ok(name)) => format!("Downloaded {name}"),
                    Some(Err(e)) => format!("Could not save: {e:?}"),
                    None => String::new(),
                };
                self.app.set_message(message);
            }
            if let Some(req) = self.app.take_print_request() {
                let message = match self.app.chart().map(|c| save_pdf(c, req)) {
                    Some(Ok(name)) => format!("Downloaded {name}"),
                    Some(Err(e)) => format!("Could not save: {e:?}"),
                    None => String::new(),
                };
                self.app.set_message(message);
            }
        }
    }

    fn save_pdf(chart: &yarnfoundry_core::Chart, req: yarnfoundry_ui_egui::PrintRequest) -> Result<String, JsValue> {
        let opts = yarnfoundry_render::PrintOptions { paper: req.paper, look: req.look, title: "YarnFoundry chart".to_string() };
        let pages = yarnfoundry_render::pages(chart, &opts).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let name = format!("yarnfoundry-chart-{}.pdf", if req.paper == yarnfoundry_render::Paper::A4 { "a4" } else { "letter" });
        download(&name, "application/pdf", &yarnfoundry_render::pdf::to_pdf(&pages, "YarnFoundry chart"))?;
        Ok(name)
    }

    fn save_project(chart: &yarnfoundry_core::Chart) -> Result<String, JsValue> {
        let json = yarnfoundry_core::to_json(chart).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let name = "yarnfoundry-chart.yarnfoundry".to_string();
        download(&name, "application/json", json.as_bytes())?;
        Ok(name)
    }

    /// Hands the browser a file to save.
    fn download(name: &str, mime: &str, bytes: &[u8]) -> Result<(), JsValue> {
        let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| JsValue::from_str("no document"))?;
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let link: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
        link.set_href(&url);
        link.set_download(name);
        link.click();
        web_sys::Url::revoke_object_url(&url)
    }

    /// Opens the browser's file chooser; the chosen picture's bytes arrive in `inbox`.
    fn choose_picture(inbox: Inbox, ctx: egui::Context) -> Result<(), JsValue> {
        let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| JsValue::from_str("no document"))?;
        let input: web_sys::HtmlInputElement = document.create_element("input")?.dyn_into()?;
        input.set_type("file");
        input.set_accept("image/png,image/jpeg,image/webp");
        let on_change = {
            let input = input.clone();
            Closure::once_into_js(move || {
                let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(buffer) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                        *inbox.borrow_mut() = Some(js_sys::Uint8Array::new(&buffer).to_vec());
                        ctx.request_repaint();
                    }
                });
            })
        };
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
        Ok(())
    }

    /// Starts the app on the canvas with this id.
    #[wasm_bindgen]
    pub async fn start(canvas_id: &str) -> Result<(), JsValue> {
        let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: web_sys::HtmlCanvasElement =
            document.get_element_by_id(canvas_id).ok_or_else(|| JsValue::from_str("canvas not found"))?.dyn_into()?;
        let mut app = YarnFoundryApp::new();
        app.set_can_read_paths(false);
        eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(move |_cc| Ok(Box::new(Host { app, inbox: Rc::new(RefCell::new(None)) }))))
            .await
    }
}
