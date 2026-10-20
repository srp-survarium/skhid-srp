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
      
      override public function set enabled(value:Boolean) : void
      {
         super.enabled = value;
         mouseEnabled = true;
      }
      
      override protected function handleMouseRelease(event:MouseEvent) : void
      {
         super.handleMouseRelease(event);
         if(enabled)
         {
            ExternalInterface.call("sound_play",0);
         }
      }
      
      override protected function setState(state:String) : void
      {
         _state = state;
         var prefixes:Vector.<String> = getStatePrefixes();
         var states:Array = _stateMap[state];
         if(states == null || states.length == 0)
         {
            return;
         }
         var l:uint = prefixes.length;
         for(var i:uint = 0; i < l; i++)
         {
            var prefix:String;
            var sl:uint;
            var j:uint;
            var thisLabel:String;
            prefix = prefixes[i];
            sl = states.length;
            for(j = 0; j < sl; j++)
            {
               thisLabel = prefix + states[j];
               if(_labelHash[thisLabel])
               {
                  _newFrame = thisLabel;
                  invalidateState();
                  return;
               }
            }
         }
      }
      
      override protected function handleMouseRollOut(event:MouseEvent) : void
      {
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var index:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         _mouseOver = false;
         if(event.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OUT));
            if(Boolean(_mouseDown & 1 << index))
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
      
      override protected function handleMouseRollOver(event:MouseEvent) : void
      {
         if(enabled)
         {
            ExternalInterface.call("sound_play",1);
         }
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         _mouseOver = true;
         if(event.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OVER));
            if(!enabled)
            {
               return;
            }
            if(lockDragStateChange && Boolean(_mouseDown << mouseIdx & 1))
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

