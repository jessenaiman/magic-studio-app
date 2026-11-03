//! `craft-animation-review`: CLI tool and script driving the ArtCraft suite to review
//! 2D-to-3D animation across types, actions, and motions.

use std::path::PathBuf;

use clap::Parser;
use magicstudio_engine::character::review::{
    AnimationReviewPipeline, ReviewContext, ReviewVerdict,
};

#[derive(Parser)]
#[command(
    name = "craft-animation-review",
    about = "Review 2D-to-3D character animation across types, actions, and motions using the ArtCraft suite"
)]
struct Args {
    /// Project root path containing character assets and comps
    #[arg(default_value = ".")]
    project: PathBuf,

    /// Character identifier (e.g. toasty)
    #[arg(long, default_value = "toasty")]
    character: String,

    /// Execute live frame renders via effectcraft-cli
    #[arg(long)]
    render_frames: bool,

    /// Custom directory to save rendered test frames
    #[arg(long)]
    render_dir: Option<PathBuf>,

    /// Print raw JSON instead of formatted terminal report
    #[arg(long)]
    json: bool,
}

fn main() {
    let args = Args::parse();
    let project = std::fs::canonicalize(&args.project).unwrap_or(args.project);
    let render_dir = args
        .render_dir
        .unwrap_or_else(|| project.join("renders/review"));

    let ctx = ReviewContext {
        project_dir: project,
        character_id: args.character,
        render_test_frames: args.render_frames,
        output_render_dir: render_dir,
    };

    let pipeline = AnimationReviewPipeline::new();
    let report = match pipeline.execute(&ctx) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error executing review pipeline: {e}");
            std::process::exit(1);
        }
    };

    if args.json {
        let json_str = serde_json::to_string_pretty(&report).unwrap_or_default();
        println!("{json_str}");
    } else {
        print!("{}", pipeline.format_terminal(&report));
    }

    if report.overall_verdict == ReviewVerdict::Fail {
        std::process::exit(1);
    }
}
