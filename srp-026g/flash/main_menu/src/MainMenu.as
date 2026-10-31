package
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.ui.Keyboard;
   import scaleform.clik.layout.Layout;
   import scaleform.clik.layout.LayoutData;
   import survarium.controls.Message;
   import survarium.menu.ActionMenu;
   import survarium.menu.BindKeyRender;
   import survarium.menu.Settings;
   import survarium.messages.MessageApply;
   import survarium.messages.MessageApplySettings;
   import survarium.messages.MessageBase;
   import survarium.messages.MessageReassign;
   
   public class MainMenu extends MovieClip
   {
      
      public var action_select:ActionMenu = new SrvActionMenu();
      
      public var settings_window:Settings = new SrvSettingsWndow();
      
      protected var layout:Layout;
      
      private var messId:int = -1;
      
      public function MainMenu()
      {
         super();
         this.init();
      }
      
      protected function init() : void
      {
         addChild(this.action_select);
         addChild(this.settings_window);
         this.settings_window.visible = false;
         var _spr:Sprite = new Sprite();
         stage.addChild(_spr);
         Message.initStage(_spr);
         Message.registerBackground(BackBlack);
         Message.registerMessage("apply",MessageApply);
         Message.registerMessage("reassign",MessageReassign);
         Message.registerMessage("applySettings",MessageApplySettings);
         Message.registerMessage("enterKeyBind",MessageEnterAnyKey);
         Message.registerMessage("promo",SrvMessageEnterPromo);
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.onKeyDownK);
         addEventListener(BindKeyRender.START_KEYBIND,this.onStartKeybind,true);
         this.setlayout();
         stage.addEventListener(Event.RESIZE,this.onStageResize);
         this.onStageResize();
      }
      
      private function onStartKeybind(e:Event) : void
      {
         trace("onStartKeybind::",e.target,e.currentTarget);
         var edata:String = e.target && e.target.data ? e.target.data.label : "nun";
         messId = (Message.showMessage("enterKeyBind",{
            textLabel:Settings.get_label("bind_text"),
            textActionLabel:edata
         }) as MessageBase).getMessageId();
      }
      
      public function show_settings(bool:Boolean) : void
      {
         if(bool)
         {
            this.settings_window.visible = true;
            this.action_select.visible = false;
         }
         else
         {
            this.settings_window.hide();
            this.action_select.visible = true;
         }
      }
      
      public function set_labels(value:Array) : void
      {
         this.settings_window.setLabels(value);
      }
      
      public function set_values(index:uint, value:Array, is_default:Boolean = true) : void
      {
         this.settings_window.setValues(index,value,is_default);
      }
      
      public function set_value(index:uint, id:uint, value:Number, is_default:Boolean = true) : void
      {
         this.settings_window.setValue(index,id,value,is_default);
      }
      
      public function set_data_provider(index:uint, id:uint, value:Array) : void
      {
         this.settings_window.setDataProvider(index,id,value);
      }
      
      public function set_settings(index:uint, value:Array) : void
      {
         this.settings_window.setSettings(index,value);
      }
      
      public function set_options(value:Array, promo:Boolean = false) : void
      {
         this.action_select.setData(value,promo);
      }
      
      // The exe's answer to the Language choice (patch 79, Settings.setLanguage).
      public function srp_language(... values) : void
      {
         this.settings_window.setLanguage(values);
      }
      
      // The exe's answer to a checkbox it keeps in user.cfg (patch 83,
      // Settings.setCheckOption): its name, label and saved state.
      public function srp_check_option(... values) : void
      {
         this.settings_window.setCheckOption(values);
      }
      
      // The exe's answer with the Settings' HUD group's texts (patch 83, Settings.setHudTexts).
      public function srp_hud_texts(... values) : void
      {
         this.settings_window.setHudTexts(values);
      }
      
      public function handle_promo_key_validation(value:Boolean, text:String) : void
      {
         this.action_select.handlePromoKeyValidation(value,text);
      }
      
      public function set_keybindings(value:Array) : void
      {
         trace("settings_window",this.settings_window);
         this.settings_window.setKeybindings(value);
      }
      
      public function set_keybind(value:uint, char:String, is_default:Boolean = true) : void
      {
         this.settings_window.setKeybind(value,char,is_default);
      }
      
      public function show_reassign_message(value:String) : void
      {
         stage.focus = null;
         Message.showMessage("reassign",{
            text:value,
            label_ok:Settings.get_label("button_ok"),
            label_cancel:Settings.get_label("button_cancel")
         },true);
      }
      
      public function end_keybind() : void
      {
         dispatchEvent(new Event(Event.CANCEL));
         if(this.messId != -1)
         {
            Message.close(this.messId);
            messId = -1;
         }
      }
      
      protected function setlayout() : void
      {
         this.action_select.layoutData = new LayoutData();
         this.action_select.layoutData.alignV = "center";
         this.action_select.layoutData.alignH = "center";
         this.action_select.layoutData.offsetV = 0;
         this.action_select.layoutData.offsetH = 0;
         this.settings_window.layoutData = new LayoutData();
         this.settings_window.layoutData.alignV = "center";
         this.settings_window.layoutData.alignH = "center";
         this.settings_window.layoutData.offsetV = 0;
         this.settings_window.layoutData.offsetH = 0;
         layout = new Layout();
         this.layout.tiedToStageSize = true;
         addChild(this.layout);
      }
      
      // The lobby and the HUD scale themselves from 1920 x 1080 (by the width up
      // to an aspect of 1.85, by the height above); this menu was drawn 1:1, half
      // its 1080p size on a 4K screen. It takes their scale now (2 at 4K), never
      // below 1: at 1080p and below it is as it was. The layout centres the Esc
      // menu and Settings in the scaled stage; the message boxes are scaled too.
      protected function onStageResize(e:Event = null) : void
      {
         var w:Number = stage.stageWidth;
         var h:Number = stage.stageHeight;
         var factor:Number = w / h < 1.85 ? w / 1920 : h / 1080;
         if(!(factor > 1))
         {
            factor = 1;
         }
         scaleX = scaleY = factor;
         Message.setScaleFactor(factor);
         this.layout.setScaleFactor(factor);
      }
      
      protected function onKeyDownK(e:KeyboardEvent) : void
      {
         var mc:MovieClip;
         if(e.keyCode == Keyboard.ESCAPE)
         {
            mc = stage.getChildByName("screenAdjust") as MovieClip;
            if(Message.isAnyMessage() != -1)
            {
               Message.close(Message.isAnyMessage());
            }
            else if(mc && mc.visible)
            {
               mc.btn_set_action();
            }
            else if(this.settings_window.visible)
            {
               this.show_settings(false);
            }
            else
            {
               this.action_select.simButton(0);
            }
         }
      }
   }
}

