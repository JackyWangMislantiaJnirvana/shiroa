use std::path::{Path, PathBuf};

use typst_ts_compiler::{
    service::{CompileDriver, DynamicLayoutCompiler},
    TypstSystemWorld,
};
use typst_ts_core::{config::CompileOpts, path::PathClean};

use crate::{
    utils::{make_absolute, UnwrapOrExit},
    CompileArgs,
};

pub struct TypstRenderer {
    pub compiler: DynamicLayoutCompiler<CompileDriver>,
    pub root_dir: PathBuf,
    pub dest_dir: PathBuf,
}

impl TypstRenderer {
    pub fn new(args: CompileArgs) -> Self {
        let workspace_dir = make_absolute(Path::new(&args.workspace)).clean();
        let root_dir = make_absolute(Path::new(&args.dir)).clean();

        let world = TypstSystemWorld::new(CompileOpts {
            root_dir: workspace_dir.clone(),
            font_paths: args.font_paths.clone(),
            //  with_embedded_fonts: EMBEDDED_FONT.to_owned(),
            ..CompileOpts::default()
        })
        .unwrap_or_exit();

        let driver = CompileDriver {
            world,
            entry_file: Default::default(),
        };

        let driver = DynamicLayoutCompiler::new(driver, Default::default()).with_enable(true);

        Self {
            compiler: driver,
            root_dir,
            dest_dir: Path::new(&args.dest_dir).to_owned(),
        }
    }

    pub fn set_entry_file(&mut self, path: &Path) {
        if path.is_absolute() {
            panic!("entry file must be relative to the workspace");
        }
        self.compiler.compiler.entry_file = self.root_dir.join(path).clean();
        let output_path = self.dest_dir.join(path).with_extension("").clean();
        self.compiler.set_output(output_path);
    }
}
