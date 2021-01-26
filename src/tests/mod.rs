use anyhow::{bail, Context, Result};
use helpers::project_root_uri;
use languageserver_types::Position;

use self::helpers::Elements;

mod helpers;

mod typescript {
    use super::{
        assert_definition,
        helpers::{get_elements, project_root_uri},
    };
    use crate::protocol::types::Language;

    #[test]
    fn test_def_var() {
        let elements = get_elements(Language::TypeScript);
        assert_definition(&elements, "TypeScript/index.ts", (2, 12), (0, 4)).unwrap();
    }

    #[test]
    fn test_def_func() {
        let elements = get_elements(Language::TypeScript);
        assert_definition(&elements, "TypeScript/index.ts", (8, 0), (4, 9)).unwrap();
    }

    #[test]
    fn test_def_func_arg() {
        let elements = get_elements(Language::TypeScript);
        assert_definition(&elements, "TypeScript/index.ts", (5, 11), (4, 15)).unwrap();
    }
}

fn assert_definition(
    elements: &Elements,
    rel_file_path: &str,
    sym_pos: (u64, u64),
    def_pos: (u64, u64),
) -> Result<()> {
    let (_range, id) = elements
        .find_range(
            &format!(
                "{}/src/tests/test_data/{}",
                project_root_uri(),
                rel_file_path
            ),
            sym_pos,
        )
        .context("Could not find target range")?;

    let def_range = {
        let defs = elements.find_definition_ranges(id);
        defs.first()
            .context("Expected find at least one definition")?
            .clone()
    };

    let expected_pos = Position {
        line: def_pos.0,
        character: def_pos.1,
    };

    if expected_pos != def_range.start {
        bail!(format!(
            "Wrong definition position. Expected {:?} got {:?}",
            expected_pos, def_range.start
        ));
    }

    Ok(())
}
