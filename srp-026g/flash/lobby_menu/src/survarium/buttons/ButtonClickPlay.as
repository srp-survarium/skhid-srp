package survarium.buttons
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.MouseEventEx;
   import survarium.constants.Events;
   import survarium.interfaces.ISubSystem;
   
   public class ButtonClickPlay extends Button implements ISubSystem
   {
      
      protected var _mouseOver:Boolean = false;
      
      public var textField2:TextField;
      
      private var _subsystemName:String;
      
      public function ButtonClickPlay()
      {
         super();
         _stateMap = {
            up:["up"],
            over:["over"],
            down:["down"],
            release:["over"],
            out:["up"],
            disabled:["disabled"],
            selecting:["over"],
            toggle:["up"],
            kb_selecting:["up"],
            kb_release:["up"],
            kb_down:["down"]
         };
      }
      
      public function get subsystem_name() : String
      {
         return this._subsystemName;
      }
      
      public function set subsystem_name(param1:String) : void
      {
         _subsystemName = param1;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         if(param1)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ENABLED));
         }
         else
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DISABLED));
         }
         super.enabled = param1;
      }
      
      override public function set visible(param1:Boolean) : void
      {
         if(!param1)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DEACTIVATED));
         }
         super.visible = param1;
      }
      
      public function get subsystem_enable() : Boolean
      {
         return enabled;
      }
      
      public function set subsystem_enable(param1:Boolean) : void
      {
         enabled = param1;
      }
      
      public function get subsystem_active() : Boolean
      {
         return visible && parent;
      }
      
      public function set subsystem_active(param1:Boolean) : void
      {
         dispatchEvent(new Event(Events.SUBSYSTEM_ACTIVATED));
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         constraintsDisabled = true;
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         textField.text = _label;
         this.textField2.text = _label;
      }
      
      override protected function handleMouseRelease(param1:MouseEvent) : void
      {
         super.handleMouseRelease(param1);
         ExternalInterface.call("sound_play",3);
      }
      
      override protected function setState(param1:String) : void
      {
         if(_data && _data is Object)
         {
            _state = param1;
         }
         if(param1 == "down")
         {
            ExternalInterface.call("sound_play",2);
         }
         var _loc2_:Vector.<String> = getStatePrefixes();
         var _loc3_:Array = _stateMap[param1];
         if(_loc3_ == null || _loc3_.length == 0)
         {
            return;
         }
         var _loc4_:uint = _loc2_.length;
         var _loc5_:uint = 0;
         var _loc6_:String;
         var _loc7_:uint;
         var _loc8_:uint;
         var _loc9_:String;
         while(_loc5_ < _loc4_)
         {
            _loc6_ = _loc2_[_loc5_];
            _loc7_ = _loc3_.length;
            _loc8_ = 0;
            while(_loc8_ < _loc7_)
            {
               _loc9_ = _loc6_ + _loc3_[_loc8_];
               if(_labelHash[_loc9_])
               {
                  _newFrame = _loc9_;
                  invalidateState();
                  return;
               }
               _loc8_++;
            }
            _loc5_++;
         }
      }
      
      override protected function handleMouseRollOut(param1:MouseEvent) : void
      {
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         _mouseOver = false;
         if(param1.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OUT));
            if(Boolean(_mouseDown & 1 << _loc3_))
            {
               if(stage != null)
               {
                  stage.addEventListener(MouseEvent.MOUSE_UP,handleReleaseOutside,false,0,true);
               }
            }
            if(lockDragStateChange || !enabled)
            {
               return;
            }
            this.setState("out");
         }
         else
         {
            if(!enabled)
            {
               return;
            }
            this.setState("out");
         }
      }
      
      override protected function handleMouseRollOver(param1:MouseEvent) : void
      {
         ExternalInterface.call("sound_play",1);
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         _mouseOver = true;
         if(param1.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OVER));
            if(!enabled)
            {
               return;
            }
            if(lockDragStateChange && Boolean(_mouseDown << _loc3_ & 1))
            {
               return;
            }
            if(_focused || _displayFocus)
            {
               this.setState(focusIndicator == null ? "down" : "kb_down");
            }
            else
            {
               this.setState("over");
            }
         }
         else
         {
            if(!enabled)
            {
               return;
            }
            this.setState("over");
         }
      }
   }
}

