use ort::session::Session;
use ort::value::Value;
use ndarray::Array;

fn main() -> anyhow::Result<()> {
    println!("\n=== Testing latent_denoiser input types ===");
    let mut session = Session::builder()?.commit_from_file("cache/latent_denoiser.onnx")?;
    
    // Try to run with dummy data to see what types are expected
    let noisy_latents: Array<f32, ndarray::Ix3> = Array::zeros((1, 144, 10));
    let encoder_outputs: Array<i64, ndarray::Ix3> = Array::zeros((1, 101, 256));  // Try i64
    let latent_mask: Array<f32, ndarray::Ix2> = Array::zeros((1, 10));
    let attention_mask: Array<i64, ndarray::Ix2> = Array::zeros((1, 101));
    let timestep: i64 = 0;
    let num_inference_steps: i64 = 4;
    let style: Array<f32, ndarray::Ix3> = Array::zeros((1, 101, 128));
    
    let result = session.run(ort::inputs! {
        "noisy_latents" => &Value::from_array(noisy_latents)?,
        "encoder_outputs" => &Value::from_array(encoder_outputs)?,
        "latent_mask" => &Value::from_array(latent_mask)?,
        "attention_mask" => &Value::from_array(attention_mask)?,
        "timestep" => &Value::from_array(ndarray::arr0(timestep))?,
        "num_inference_steps" => &Value::from_array(ndarray::arr0(num_inference_steps))?,
        "style" => &Value::from_array(style)?
    });
    
    match result {
        Ok(_) => println!("Success!"),
        Err(e) => println!("Error: {:?}", e),
    }
    
    Ok(())
}
