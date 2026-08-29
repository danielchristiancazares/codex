use super::*;
use pretty_assertions::assert_eq;

#[test]
fn compact_menu_layout_is_complete_for_heights_zero_through_seven() {
    let expected_rows: [(
        Option<u16>,
        Option<u16>,
        [Option<u16>; 2],
        Option<u16>,
        bool,
    ); 8] = [
        (None, None, [None, None], None, false),
        (Some(0), None, [None, None], None, false),
        (None, None, [Some(0), Some(1)], None, false),
        (None, None, [Some(0), Some(1)], Some(2), false),
        (None, Some(0), [Some(1), Some(2)], Some(3), false),
        (None, Some(0), [Some(2), Some(3)], Some(4), false),
        (None, Some(0), [Some(2), Some(3)], Some(5), false),
        (None, Some(1), [Some(3), Some(4)], Some(6), true),
    ];

    for (height, (compact, instruction, options, guidance, full_guidance)) in
        expected_rows.into_iter().enumerate()
    {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 60, height as u16);
        let layout = ModelMigrationLayout::new(area, /*copy_preferred_height*/ 20);
        let rect = |row| row.map(|row| Rect::new(0, row, 60, 1));

        assert_eq!(
            layout,
            ModelMigrationLayout {
                copy_area: Rect::new(0, 0, 60, 0),
                compact_options_area: rect(compact),
                instruction_area: rect(instruction),
                option_areas: options.map(|row| rect(row)),
                guidance_area: rect(guidance),
                full_guidance,
            },
            "height {height}",
        );
    }
}
