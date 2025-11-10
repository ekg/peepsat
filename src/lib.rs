use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;

#[wasm_bindgen]
pub struct WgpuApp {
    canvas: web_sys::HtmlCanvasElement,
    context: Option<CanvasRenderingContext2d>,
}

#[wasm_bindgen]
impl WgpuApp {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: web_sys::HtmlCanvasElement) -> WgpuApp {
        WgpuApp {
            canvas,
            context: None,
        }
    }

    #[wasm_bindgen]
    pub fn init(&mut self) -> Result<(), JsValue> {
        let context_obj = self.canvas.get_context("2d").map_err(|_| "Failed to get 2d context")?;
        let context = context_obj.ok_or("Context is None")?.dyn_into::<CanvasRenderingContext2d>().map_err(|_| "Failed to cast context")?;
        self.context = Some(context);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn render(&mut self) -> Result<(), JsValue> {
        let context = self.context.as_ref().unwrap();
        context.set_fill_style(&"black".into());
        let width = self.canvas.width() as f64;
        let height = self.canvas.height() as f64;
        context.fill_rect(0.0, 0.0, width, height);
        Ok(())
    }
}

fn create_sphere(radius: f32, stacks: u32, slices: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=stacks {
        let phi = (i as f32 / stacks as f32) * std::f32::consts::PI;
        for j in 0..=slices {
            let theta = (j as f32 / slices as f32) * 2.0 * std::f32::consts::PI;
            let x = radius * phi.sin() * theta.cos();
            let y = radius * phi.cos();
            let z = radius * phi.sin() * theta.sin();
            vertices.push([x, y, z]);
        }
    }

    for i in 0..stacks {
        for j in 0..slices {
            let first = i * (slices + 1) + j;
            let second = first + slices + 1;
            indices.extend_from_slice(&[first, second, first + 1, second, second + 1, first + 1]);
        }
    }

    (vertices, indices)
}