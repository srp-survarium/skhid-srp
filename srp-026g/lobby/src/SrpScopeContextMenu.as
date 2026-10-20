package {
    import survarium.upgrade.UpgradeWindow;

    public class SrpScopeContextMenu {
        public static function isUpgradeable(item:Object):Boolean {
            if (!item) return false;
            var category:int = int(item.category);
            return ((category == 3 || category == 1 || category == 5 || category == 4)
                    && (Boolean(item.camoAvailable) || Boolean(item.decalAvailable)))
                || (UpgradeWindow.isWeaponCategory(category)
                    && UpgradeWindow.hasScopes(int(item.dictId)));
        }

        public static function labelKey(item:Object):String {
            return UpgradeWindow.isWeaponCategory(int(item.category))
                ? "scope_window_title" : "btn_upgrade_label";
        }
    }
}
