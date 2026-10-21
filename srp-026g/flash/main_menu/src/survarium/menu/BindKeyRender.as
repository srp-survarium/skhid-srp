package survarium.menu
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   
   public class BindKeyRender extends ListItemRenderer
   {
      
      public static const START_KEYBIND:String = "startBindKey";
      
      public var btn_key:Button;
      
      public var chk_toggle:Button;
      
      public var back:Sprite;
      
      protected var _enter_mode:Boolean = false;
      
      public function BindKeyRender()
      {
         super();
      }
      
      override public function set selected(value:Boolean) : void
      {
         if(!_data)
         {
            return;
         }
         super.selected = value;
         if(!value)
         {
            this.btn_key.selected = false;
            this.btn_key.displayFocus = false;
            this.chk_toggle.displayFocus = false;
            this.btn_key.invalidateState();
            this.changeFocus();
         }
      }
      
      override public function set data(value:Object) : void
      {
         if(!value)
         {
            return;
         }
         if(value.defKey == undefined)
         {
            value.defKey = value.key;
         }
         if(value.defToggle == undefined)
         {
            value.defToggle = value.toggle;
         }
         _data = value;
         if(value.enter)
         {
            dispatchEvent(new Event(Event.CHANGE,true));
            _enter_mode = false;
            value.enter = false;
         }
         invalidate(InvalidationType.DATA);
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
         {
            return;
         }
         event.preventDefault();
         var details:InputDetails = event.details;
         var controllerIndex:uint = details.controllerIndex;
         switch(details.code)
         {
            case Keyboard.ENTER:
               if(details.value == InputValue.KEY_DOWN)
               {
                  if(!this._enter_mode)
                  {
                     if(this.btn_key.displayFocus == true)
                     {
                        this.startKey(null);
                     }
                     else if(this.chk_toggle.displayFocus == true)
                     {
                        this.chk_toggle.selected = !this.chk_toggle.selected;
                        this.onCheckBox();
                     }
                  }
                  else
                  {
                     this.endKey();
                  }
                  event.handled = true;
               }
               break;
            case Keyboard.LEFT:
               if(details.value == InputValue.KEY_DOWN && this.chk_toggle.displayFocus == false && !this._enter_mode && this.chk_toggle.visible)
               {
                  this.btn_key.displayFocus = false;
                  this.chk_toggle.displayFocus = true;
                  event.handled = true;
               }
               break;
            case Keyboard.RIGHT:
               if(details.value == InputValue.KEY_DOWN && this.btn_key.displayFocus == false)
               {
                  this.btn_key.displayFocus = true;
                  this.chk_toggle.displayFocus = false;
                  event.handled = true;
               }
               break;
            case Keyboard.ESCAPE:
               if(details.value == InputValue.KEY_DOWN)
               {
                  this.endKey();
                  event.handled = true;
               }
               break;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         constraintsDisabled = true;
         textField.tabEnabled = textField.mouseEnabled = false;
         this.btn_key.enabled = enabled;
         this.btn_key.autoRepeat = false;
         this.btn_key.focusable = false;
         this.btn_key.focusTarget = this;
         this.btn_key.tabEnabled = false;
         this.btn_key.mouseEnabled = true;
         this.chk_toggle.enabled = enabled;
         this.chk_toggle.autoRepeat = false;
         this.chk_toggle.focusTarget = this;
         this.chk_toggle.tabEnabled = false;
         this.chk_toggle.mouseEnabled = true;
         mouseChildren = true;
         textField.mouseEnabled = false;
         this.back.mouseEnabled = false;
         this.back.mouseChildren = false;
         doubleClickEnabled = true;
         addEventListener(MouseEvent.DOUBLE_CLICK,this.startKey);
         this.btn_key.addEventListener(MouseEvent.CLICK,this.startKey);
         this.btn_key.addEventListener(MouseEvent.ROLL_OVER,this.overBtn);
         this.btn_key.addEventListener(MouseEvent.ROLL_OUT,this.upBtn);
         this.chk_toggle.addEventListener(ButtonEvent.CLICK,this.onCheckBox);
      }
      
      private function overBtn(event:MouseEvent) : void
      {
         this.btn_key.gotoAndStop("over_k");
      }
      
      private function upBtn(event:MouseEvent) : void
      {
         this.btn_key.gotoAndStop("up_k");
      }
      
      override protected function changeFocus() : void
      {
         if(_focused || _displayFocus)
         {
            setState("focused");
         }
         else
         {
            setState("default");
         }
         if(_displayFocus)
         {
            setState("over");
            this.btn_key.selected = true;
            this.btn_key.displayFocus = true;
         }
         else
         {
            setState("up");
            this.btn_key.displayFocus = false;
         }
      }
      
      override protected function updateText() : void
      {
         mouseChildren = true;
         if(!data)
         {
            return;
         }
         textField.text = data.label;
         if(isInvalid(InvalidationType.DATA))
         {
            this.btn_key.label = data.key;
            this.btn_key.gotoAndStop("up_k");
         }
         if(_data.toggle == undefined)
         {
            this.chk_toggle.visible = false;
         }
         else
         {
            this.chk_toggle.visible = true;
            this.chk_toggle.selected = _data.toggle;
         }
         this.btn_key.enabled = _data.type != 0;
      }
      
      protected function startKey(event:Event) : void
      {
         trace("startKey::",event ? event.type : "noevent");
         if(!this.btn_key.enabled)
         {
            return;
         }
         this.btn_key.gotoAndStop("down_k");
         ExternalInterface.call("start_bind_key",data.action_id);
         _enter_mode = true;
         stage.addEventListener(Event.CANCEL,this.onMe,true);
         dispatchEvent(new Event(START_KEYBIND));
      }
      
      protected function onMe(event:Event = null) : void
      {
         this.endKey();
      }
      
      protected function endKey(event:Event = null) : void
      {
         var list:TileList;
         if(this._enter_mode == false)
         {
            return;
         }
         if(stage)
         {
            stage.removeEventListener(Event.CANCEL,this.onMe);
         }
         if(list)
         {
            stage.focus = list;
            list.selectedIndex = index;
         }
         if(data && data.key == undefined)
         {
            data.key = "LOL";
         }
         if(data)
         {
            ExternalInterface.call("end_bind_key",index,data.name);
            this.btn_key.gotoAndStop("up_k");
            this.btn_key.label = data.key;
         }
         list = this.parent.parent as TileList;
         dispatchEvent(new Event(Event.CHANGE,true));
         _enter_mode = false;
      }
      
      protected function onCheckBox(event:ButtonEvent = null) : void
      {
         _data.toggle = this.chk_toggle.selected;
         dispatchEvent(new Event(Event.CHANGE,true));
      }
   }
}

