package
{
   import com.greensock.TweenLite;
   import flash.display.Bitmap;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.utils.setTimeout;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.CheckBox;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.layout.Layout;
   import scaleform.clik.layout.LayoutData;
   import scaleform.gfx.FocusManager;
   import scaleform.gfx.MouseCursorEvent;
   import survarium.controls.Message;
   import survarium.controls.SrvTextInput;
   import survarium.data.LocalizationData;
   import survarium.messages.MessageApply;
   import survarium.messages.MessageEULA;

   config namespace KEY;
   KEY const ENTER:int = 13;
   KEY const ESCAPE:int = 27;
   
   public class LoginMenu extends MovieClip
   {
      
      public static var scaleFactor:Number = 1;
      
      public var status:TextField;
      
      public var status_str:TextField;
      
      public var registration_str:TextField;
      
      public var login_input:SrvTextInput;
      
      public var password_input:SrvTextInput;
      
      public var sign_in_btn:Button;
      
      public var register_btn:Button;
      
      public var exit_btn:Button;
      
      public var save_checkbox:CheckBox;
      
      public var loadingIndicator:MovieClip = new LoadingIndicator();
      
      public var panelContainer:MovieClip = new PanelContainer();
      
      public var tweenLite:TweenLite;
      
      protected var _layout:Layout;
      
      protected var _superHeight:uint = 1080;
      
      protected var _superWidth:uint = 1920;
      
      public function LoginMenu()
      {
         super();
         updateFocus = function():void
         {
            FocusManager.moveFocus("tab");
         };
         sign_in_button_clicked = function(e:ButtonEvent):void
         {
            loadingIndicator.gotoAndPlay(1);
            sign_in_btn.addEventListener(ComponentEvent.STATE_CHANGE,onBtnUpdate);
            callSignIn();
            status.alpha = 1;
         };
         onKeyDownK = function(e:KeyboardEvent):void
         {
            if(e.keyCode == KEY::ESCAPE)
            {
               if(Message.isAnyMessage() != -1)
               {
                  Message.close(Message.isAnyMessage());
               }
               else
               {
                  exit_button_clicked();
               }
            }
         };
         exit_button_clicked = function(e:ButtonEvent = null):void
         {
            Message.showMessage("ask",{
               text:LocalizationData.getName("exit_game_confirm_text"),
               onYes:callExit,
               label_ok:LocalizationData.getName("exit_game_confirm_accept"),
               label_cancel:LocalizationData.getName("exit_game_confirm_cancel")
            });
         };
         callExit = function():void
         {
            ExternalInterface.call("exit_button_clicked");
         };
         try_enter_pass = function(e:KeyboardEvent):void
         {
            e.preventDefault();
            if(e.keyCode == KEY::ENTER && sign_in_btn.enabled)
            {
               sign_in_button_clicked(null);
            }
         };
         clearPass = function():void
         {
            password_input.text = "";
         };
         selectAll = function(e:Event):void
         {
            var tx:TextInput = e.currentTarget as TextInput;
            if(tx.focused)
            {
               tx.textField.setSelection(0,tx.length);
            }
         };
         trace("LoginMenu::super");
         var format1:TextFormat = new TextFormat("Calibri",18,16777215,true);
         var format2:TextFormat = new TextFormat("Calibri",18,16777215,true);
         var spriteMail:Sprite = this.panelContainer.spriteMail;
         var spritePass:Sprite = this.panelContainer.spritePass;
         loadingIndicator = this.panelContainer.loadingIndicator;
         spriteMail.mouseEnabled = spritePass.mouseEnabled = false;
         this.loadingIndicator.gotoAndStop(1);
         var backContainer:MovieClip = new MovieClip();
         addChild(backContainer);
         var back:Bitmap = new Bitmap(new LoginBackground());
         backContainer.addChild(back);
         exit_btn = new ButtonCloseWindow2();
         this.panelContainer.alpha = 0;
         addChild(this.panelContainer);
         addEventListener(Event.RENDER,this.configUI);
         var shift:uint = 20;
         save_checkbox = this.panelContainer.save_checkbox;
         sign_in_btn = this.panelContainer.sign_in_btn;
         login_input = this.panelContainer.login_input;
         this.login_input.maxChars = 120;
         password_input = this.panelContainer.password_input;
         this.password_input.maxChars = 120;
         this.password_input.displayAsPassword = true;
         status_str = this.panelContainer.status;
         status = this.panelContainer.status;
         this.status.alpha = 0.2;
         register_btn = this.panelContainer.register_btn;
         this.register_btn.addEventListener(ButtonEvent.CLICK,this.onRegistrationClicked);
         this.status_str.autoSize = "center";
         this.loadingIndicator.alpha = 0.7;
         var spr:MovieClip = new MovieClip();
         stage.addChild(spr);
         Message.initStage(spr as Sprite);
         Message.registerBackground(BackBlack);
         Message.registerMessage("ask",MessageApply);
         Message.registerMessage("eula",MessageEULA);
         FocusManager.setModalClip(this);
         FocusManager.moveFocus("tab");
         setTimeout(updateFocus,10);
         this.login_input.invalidate(InvalidationType.ALL);
         this.sign_in_btn.addEventListener(ButtonEvent.CLICK,sign_in_button_clicked);
         stage.addEventListener(KeyboardEvent.KEY_DOWN,onKeyDownK);
         this.exit_btn.addEventListener(ButtonEvent.CLICK,exit_button_clicked);
         this.login_input.addEventListener(KeyboardEvent.KEY_DOWN,try_enter_pass);
         this.password_input.addEventListener(KeyboardEvent.KEY_DOWN,try_enter_pass);
         LocalizationData.addCallback(this.onLocal,"login_label");
         LocalizationData.addButton(this.exit_btn,"exit");
         LocalizationData.addButton(this.register_btn,"registration_str");
         LocalizationData.addButton(this.save_checkbox,"save_password_str");
         spr.layoutData = new LayoutData();
         spr.layoutData.alignV = "top";
         spr.layoutData.alignH = "left";
         spr.layoutData.offsetV = 0;
         spr.layoutData.offsetH = 0;
         this.exit_btn.layoutData = new LayoutData();
         this.exit_btn.layoutData.alignV = "top";
         this.exit_btn.layoutData.alignH = "right";
         this.exit_btn.layoutData.offsetV = 20;
         this.exit_btn.layoutData.offsetH = -20;
         this.panelContainer.layoutData = new LayoutData();
         this.panelContainer.layoutData.alignV = "top";
         this.panelContainer.layoutData.alignH = "right";
         this.panelContainer.layoutData.offsetV = -0.0;
         this.panelContainer.layoutData.offsetH = -40;
         stage.addEventListener(Event.RESIZE,this.onStageResize);
         _layout = new Layout();
         this._layout.tiedToStageSize = true;
         addChild(this._layout);
         this._layout.reflow();
         this.panelContainer.x = backContainer.width - 170 - this.panelContainer.width - 40;
         this.login_input.textField.addEventListener(Event.CHANGE,clearPass);
         this.login_input.addEventListener(ComponentEvent.STATE_CHANGE,selectAll);
         this.password_input.addEventListener(ComponentEvent.STATE_CHANGE,selectAll);
         stage.addEventListener(MouseCursorEvent.CURSOR_CHANGE,this.onChangeCursor);
         SrpLoginHint.apply(this);
         var updateFocus:Function;
         var sign_in_button_clicked:Function;
         var onKeyDownK:Function;
         var exit_button_clicked:Function;
         var callExit:Function;
         var try_enter_pass:Function;
         var clearPass:Function;
         var selectAll:Function;

      }
      
      private function onLocal(value:String) : void
      {
         this.sign_in_btn.label = value.toUpperCase();
      }
      
      public function set_status(value:String) : void
      {
         this.status.text = value;
         this.onTextUpdated();
      }
      
      private function onTextUpdated() : void
      {
         this.loadingIndicator.x = this.status_str.x + this.status_str.getLineMetrics(0).x - this.loadingIndicator.width + 5;
      }
      
      public function show_eula_wnd() : void
      {
         Message.showMessage("eula",{
            label_window:LocalizationData.getName("EULA_label"),
            text:LocalizationData.getName("EULA"),
            onYes:this.agree_eula,
            onNo:this.disagree_eula,
            label_ok:LocalizationData.getName("accept"),
            label_ok:LocalizationData.getName("label_cancel")
         });
      }
      
      public function set_localization_data(value:Object) : void
      {
         LocalizationData.setData(value);
         this.login_input.defaultText = LocalizationData.getName("login_default_text");
         this.password_input.defaultText = LocalizationData.getName("password_default_text");
         SrpLoginHint.apply(this);
      }
      
      private function onChangeCursor(e:MouseCursorEvent) : void
      {
         var _new:uint;
         if(e.cursor == "ibeam")
         {
            _new = 6;
         }
         else if(e.cursor != "arrow")
         {
            _new = 2;
         }
         else
         {
            _new = 1;
         }
         ExternalInterface.call("set_mouse_cursor",_new);
      }
      
      private function onRegistrationClicked(e:Event) : void
      {
      }
      
      private function configUI(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.configUI);
         tweenLite = TweenLite.to(this.panelContainer,0.8,{
            delay:0.5,
            alpha:1
         });
      }
      
      private function callSignIn() : void
      {
         ExternalInterface.call("sign_in_button_clicked",this.login_input.text,this.password_input.text);
      }
      
      private function onStageResize(event:Event = null) : void
      {
         trace("new_stage_values",stage.stageHeight,stage.stageWidth,stage.stageWidth / stage.stageHeight);
         if(stage.stageWidth / stage.stageHeight < 1.85)
         {
            scaleFactor = stage.stageWidth / this._superWidth;
         }
         else
         {
            scaleFactor = stage.stageHeight / this._superHeight;
         }
         scaleFactor = int(stage.stageHeight / this._superHeight / 0.01) * 0.01;
         Message.setScaleFactor(scaleFactor);
         scaleX = scaleFactor;
         scaleY = scaleFactor;
         this._layout.setScaleFactor(scaleFactor);
         this._layout.reflow();
         this.panelContainer.x = int(this.panelContainer.x);
      }
      
      private function onBtnUpdate(e:Event) : void
      {
         if(this.sign_in_btn.enabled)
         {
            this.sign_in_btn.removeEventListener(ComponentEvent.STATE_CHANGE,this.onBtnUpdate);
            this.loadingIndicator.gotoAndStop(1);
         }
      }
      
      private function agree_eula() : void
      {
         ExternalInterface.call("eula_agree_button_clicked",this.login_input.text,this.password_input.text);
      }
      
      private function disagree_eula() : void
      {
         ExternalInterface.call("eula_not_agree_button_clicked");
      }
   }
}

