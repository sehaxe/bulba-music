//! Prints the twelve chroma bins of a file (or a generated chord), so the pitch
//! reference can be checked against ground truth instead of guessed.
//!
//! `cargo run --release --bin chroma -- /tmp/c-major.wav`

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("path");
    let f = bulba_music_server::features::extract(&path)?;
    let chroma = &f.vec[40..52];
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let mut order: Vec<(usize, f32)> = chroma.iter().copied().enumerate().collect();
    order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    print!("chroma:");
    for (i, v) in &order[..4] {
        print!(" {}={:.3}", names[*i], v);
    }
    println!();
    let k = bulba_music_server::keys::detect(&chroma.try_into().unwrap());
    println!(
        "key: {} {} conf {:.2}",
        names[k.key as usize],
        if k.mode == 0 { "major" } else { "minor" },
        k.conf
    );
    Ok(())
}
