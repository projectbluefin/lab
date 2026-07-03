use fn_error_context::context;

#[context("context")]
async fn borrows(x: &mut u32) -> anyhow::Result<&mut u32> {
    Ok(x)
}

fn main() {}
