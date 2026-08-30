use super::*;

#[test]
fn cf_060_adding_one_skill_emits_only_the_added_catalog_entry() {
    let previous_lines = CatalogLines {
        roots: Vec::new(),
        skills: (0..20)
            .map(|index| format!("- skill-{index}: Existing skill {index}."))
            .collect(),
    };
    let mut current_lines = previous_lines.clone();
    current_lines
        .skills
        .push("- skill-20: Newly added skill.".to_string());
    let previous_body = format!(
        "\n## Skills\n### Available skills\n{}\n",
        previous_lines.skills.join("\n")
    );
    let current_body = format!(
        "\n## Skills\n### Available skills\n{}\n",
        current_lines.skills.join("\n")
    );
    let previous = executor_skills_world_state_section(
        Some(previous_body),
        previous_lines,
        /*include_instructions*/ true,
        Box::new(|| {}),
    );
    let current = executor_skills_world_state_section(
        Some(current_body),
        current_lines,
        /*include_instructions*/ true,
        Box::new(|| {}),
    );
    let revision = current.snapshot()["revision"]
        .as_str()
        .expect("catalog revision");

    assert_eq!(
        current
            .render_diff(PreviousWorldStateSection::Known(previous.snapshot()))
            .expect("added skill should render")
            .body(),
        format!(
            "\n## Skills update\n### Added or updated skills\n- skill-20: Newly added skill.\n<!-- skills-catalog-revision:{revision} -->\n"
        )
    );
}
