use super::*;

pub(super) fn visit_recipe(
    state: &AppState,
    repo_id: &str,
    recipe_id: &str,
    blobs: &mut HashSet<String>,
    recipes: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> Result<(), Response> {
    if !visited.insert(recipe_id.to_string()) {
        return Ok(());
    }
    recipes.insert(recipe_id.to_string());
    let recipe = read_recipe(state, repo_id, recipe_id)?;
    for c in recipe.chunks {
        blobs.insert(c.blob.as_str().to_string());
    }
    Ok(())
}
