use std::path::Path;
fn is_application_temp(value: &str, root: &Path) -> bool {
    let value = value.replace('/', "\\");
    let owned = root.join("data/temp").to_string_lossy().replace('/', "\\");
    value
        .trim_end_matches('\\')
        .eq_ignore_ascii_case(owned.trim_end_matches('\\'))
}
pub fn restore_inherited_temp(root: &Path) {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let system_temp = std::path::PathBuf::from(local).join("Temp");
    for key in ["TEMP", "TMP", "TMPDIR"] {
        if std::env::var_os(key)
            .is_some_and(|value| is_application_temp(&value.to_string_lossy(), root))
        {
            std::env::set_var(key, &system_temp);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrade_environment_recognizes_only_the_owned_temporary_folder() {
        let root = Path::new("C:/LootLazy");
        assert!(is_application_temp("c:\\lootlazy\\data\\temp\\", root));
        assert!(is_application_temp("C:/LootLazy/data/temp", root));
        assert!(!is_application_temp("C:/LootLazy/data/temp-other", root));
        assert!(!is_application_temp("C:/OtherApp/data/temp", root));
    }
    #[test]
    fn user_system_and_custom_temp_paths_are_not_replaced() {
        let root = Path::new("C:/LootLazy");
        for value in ["C:/Users/Test/AppData/Local/Temp", "D:/CustomTemp", ""] {
            assert!(!is_application_temp(value, root));
        }
    }
}
