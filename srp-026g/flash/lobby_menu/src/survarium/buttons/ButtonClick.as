package survarium.buttons
{
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.utils.Constraints;
   import scaleform.gfx.MouseEventEx;
   
   public class ButtonClick extends Button
   {
      
      protected var _mouseOver:Boolean = false;
      
      public function ButtonClick()
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
      
      override protected function updateText() : void
      {
         super.updateText();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraintsDisabled && textField)
         {
            constraints.addElement("textField",textField,Constraints.LEFT | Constraints.RIGHT);
         }
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         super.enabled = param1;
         mouseEnabled = true;
      }
      
      override protected function handleMouseRelease(param1:MouseEvent) : void
      {
         super.handleMouseRelease(param1);
         if(enabled)
         {
            ExternalInterface.call("sound_play",0);
         }
      }
      
      override protected function setState(param1:String) : void
      {
         _state = param1;
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
         if(enabled)
         {
            ExternalInterface.call("sound_play",1);
         }
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

