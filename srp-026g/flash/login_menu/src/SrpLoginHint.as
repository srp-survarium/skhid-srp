package
{
    import flash.text.TextField;
    import flash.text.TextFormat;
    import survarium.data.LocalizationData;

    public class SrpLoginHint
    {
        public static function apply(menu:LoginMenu):void
        {
            menu.register_btn.visible = false;
            menu.register_btn.enabled = false;
            menu.register_btn.mouseEnabled = false;
            menu.register_btn.mouseChildren = false;
            menu.register_btn.tabEnabled = false;

            var hint:TextField = menu.registration_str;
            if (hint == null)
            {
                hint = new TextField();
                menu.registration_str = hint;
                hint.x = menu.login_input.x;
                hint.y = menu.register_btn.y;
                hint.width = menu.login_input.width;
                hint.height = 56;
                hint.wordWrap = true;
                hint.multiline = true;
                hint.selectable = false;
                hint.mouseEnabled = false;
                hint.tabEnabled = false;
                hint.defaultTextFormat = new TextFormat("Calibri", 18, 0xFFFFFF,
                    false, false, false, null, null, "center");
                menu.panelContainer.addChild(hint);
            }
            // Match the stock localized registration string supplied by the
            // native client. This covers all five supported client languages.
            var translations:Object = {
                "Create account": "Account created on first sign-in",
                "Створити новий акаунт": "Акаунт створюється під час першого входу",
                "Создать учетную запись": "Аккаунт создаётся при первом входе",
                "Konto registrieren": "Konto wird beim ersten Login erstellt",
                "Utwórz konto": "Konto powstaje przy pierwszym logowaniu"
            };
            var text:String = translations[LocalizationData.getName("registration_str")];
            hint.text = text != null ? text : "Account created on first sign-in";
        }
    }
}
