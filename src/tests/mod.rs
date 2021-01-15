use helpers::find_range;

use crate::protocol::types::Language;

mod helpers;

mod typescript {
    use languageserver_types::Position;

    use crate::protocol::types::Language;

    use super::helpers::{find_definition_ranges, find_range, get_elements};

    #[test]
    fn test_def_1() {
        let elements = get_elements(Language::TypeScript);
        let (r, id) = find_range(
            &elements,
            "file:///Users/zas/space/lsif-os/src/tests/test_data/TypeScript/index.ts",
            (2, 12),
        )
        .expect("Could not find target range");

        let defs = find_definition_ranges(&elements, id);

        let def_range = defs.first().expect("Expected to find the definition");

        assert_eq!(
            def_range.start,
            Position {
                line: 0,
                character: 4
            }
        )
    }
}
