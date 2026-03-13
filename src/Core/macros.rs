#[macro_export]
macro_rules! export_ability {
    ($ability_ty:ident) => {
        inventory::submit!(&$ability_ty as &dyn $crate::AbilityExecutor);
    };
}

#[macro_export]
macro_rules! export_module {
    ($module_ty:ident) => {
        inventory::submit!(&$module_ty as &dyn $crate::ModuleExecutor);
    };
}
