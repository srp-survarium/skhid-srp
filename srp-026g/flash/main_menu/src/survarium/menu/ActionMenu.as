package survarium.menu
{
   import flash.display.Sprite;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonBarEvent;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.Message;
   
   public class ActionMenu extends UIComponent
   {
      
      public var background:Sprite;
      
      public var menu_buttons:ButtonBar;
      
      public var btnPromo:Button;
      
      public var label:TextField;
      
      public var tiledSprite:*;
      
      protected var _actions:Dictionary = new Dictionary();
      
      protected var _current_action:String = "";
      
      protected var _promoMessage:SrvMessageEnterPromo;
      
      public function ActionMenu()
      {
         super();
      }
      
      public function setData(buttons:Array, promoEnabled:Boolean) : void
      {
         trace("AM::setdata",this.menu_buttons,buttons,this.btnPromo);
         this.menu_buttons.dataProvider = new DataProvider(buttons);
         var buttonsHeight:uint = buttons.length * 37;
         this.menu_buttons.height = buttonsHeight;
         if(promoEnabled)
         {
            this.btnPromo.visible = true;
            this.btnPromo.y = buttonsHeight + 20;
            this.btnPromo.label = Settings.get_label("btn_promo_key_label");
            this.background.height = this.btnPromo.height + this.btnPromo.y + 14;
         }
         else
         {
            this.btnPromo.visible = false;
            this.btnPromo.y = 0;
            this.background.height = buttonsHeight + 26;
         }
         this.tiledSprite.height = this.background.height - this.tiledSprite.y;
      }
      
      public function simButton(buttonIndex:uint) : void
      {
         ExternalInterface.call("menu_button",this.menu_buttons.dataProvider[buttonIndex].action);
      }
      
      override protected function configUI() : void
      {
         this.menu_buttons.dataProvider = new DataProvider([{
            label:"назад",
            action:"close"
         },{
            label:"настройки",
            action:"settings"
         },{
            label:"logout",
            action:"logoout"
         },{
            label:"выход в виндовс",
            action:"exit"
         }]);
         this._actions["close"] = this.closeMe;
         this._actions["settings"] = this.showSettings;
         this._actions["logoout"] = this.logout;
         this._actions["exit_to_os"] = this.exitGame;
         this._actions["leave_match"] = this.exitBattle;
         this.menu_buttons.addEventListener(ButtonBarEvent.BUTTON_SELECT,this.takeAction);
         this.btnPromo.addEventListener(ButtonEvent.CLICK,this.showPromo);
         this.tiledSprite.setTile(new HintLinePattern());
      }
      
      public function handlePromoKeyValidation(isValid:Boolean, warningText:String) : void
      {
         this._promoMessage.handleCodeValidation(isValid,warningText);
      }
      
      private function showPromo(event:ButtonEvent) : void
      {
         _promoMessage = Message.showMessage("promo",{
            label_ok:Settings.get_label("button_ok"),
            default_text:Settings.get_label("enter_promo_key")
         },true);
      }
      
      protected function onClose() : void
      {
         ExternalInterface.call("menu_button","back");
      }
      
      protected function takeAction(event:ButtonBarEvent) : void
      {
         var action:String = this.menu_buttons.dataProvider[event.index].action;
         _current_action = action;
         trace("button pressed:",event.index,this.menu_buttons.dataProvider[event.index].action,this._actions[action]);
         if(this._actions[action] is Function)
         {
            trace("555");
            this._actions[action]();
         }
         else
         {
            trace("lll");
            ExternalInterface.call("menu_button",action);
         }
         this.menu_buttons.selectedIndex = -1;
      }
      
      protected function exitBattle() : void
      {
         Message.showMessage("apply",{
            text:Settings.get_label("exit_match_confirm_text"),
            label_ok:Settings.get_label("exit_match_confirm_accept"),
            label_cancel:Settings.get_label("exit_match_confirm_cancel"),
            onYes:this.action_accept
         });
      }
      
      protected function closeMe() : void
      {
         trace("HIDE MENU");
         ExternalInterface.call("hide_menu");
      }
      
      protected function showSettings() : void
      {
         parent["show_settings"](true);
      }
      
      protected function logout() : void
      {
         ExternalInterface.call("logout");
      }
      
      protected function exitGame() : void
      {
         Message.showMessage("apply",{
            text:Settings.get_label("exit_game_confirm_text"),
            label_ok:Settings.get_label("exit_game_confirm_accept"),
            label_cancel:Settings.get_label("exit_game_confirm_cancel"),
            onYes:this.action_accept
         });
      }
      
      protected function action_accept() : void
      {
         ExternalInterface.call("menu_button",this._current_action);
      }
   }
}

