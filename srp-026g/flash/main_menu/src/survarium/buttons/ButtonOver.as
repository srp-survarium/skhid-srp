package survarium.buttons
{
   import flash.display.DisplayObject;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.MouseEventEx;
   import survarium.constants.UiSound;
   
   public class ButtonOver extends Button
   {
      
      public function ButtonOver()
      {
         super();
         _stateMap = {
            up:["up"],
            over:["over"],
            down:["down"],
            release:["up"],
            out:["up"],
            disabled:["disabled"],
            selecting:["over"],
            toggle:["up"],
            kb_selecting:["up"],
            kb_release:["up"],
            kb_down:["down"]
         };
      }
      
      override public function set enabled(value:Boolean) : void
      {
         var state:String;
         if(enabled == value)
         {
            return;
         }
         super.enabled = value;
         mouseChildren = false;
         if(super.enabled)
         {
            state = "up";
         }
         else
         {
            state = "disabled";
         }
         this.setState(state);
      }
      
      override public function set selected(value:Boolean) : void
      {
         if(_selected == value)
         {
            return;
         }
         _selected = value;
         validateNow();
         dispatchEvent(new Event(Event.SELECT));
      }
      
      override protected function changeFocus() : void
      {
      }
      
      override protected function setState(stateName:String) : void
      {
         _state = stateName;
         var prefixes:Vector.<String> = getStatePrefixes();
         var states:Array = _stateMap[stateName];
         if(states == null || states.length == 0)
         {
            return;
         }
         var prefixCount:uint = prefixes.length;
         var i:uint = 0;
         var prefix:String;
         var stateCount:uint;
         var j:uint;
         var frameLabel:String;
         while(i < prefixCount)
         {
            prefix = prefixes[i];
            stateCount = states.length;
            j = 0;
            while(j < stateCount)
            {
               frameLabel = prefix + states[j];
               if(_labelHash[frameLabel])
               {
                  _newFrame = frameLabel;
                  invalidateState();
                  return;
               }
               j++;
            }
            i++;
         }
      }
      
      override protected function handleReleaseOutside(event:MouseEvent) : void
      {
         _autoRepeatEvent = null;
         if(contains(event.target as DisplayObject))
         {
            return;
         }
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         var btnIdx:uint = sfEvent == null ? 0 : sfEvent.buttonIdx;
         if(btnIdx != MouseEventEx.LEFT_BUTTON)
         {
            return;
         }
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.handleReleaseOutside,false);
         _mouseDown ^= 1 << mouseIdx;
         dispatchEvent(new ButtonEvent(ButtonEvent.RELEASE_OUTSIDE));
         if(!enabled)
         {
            return;
         }
         this.setState("release");
      }
      
      override protected function handleMouseRelease(event:MouseEvent) : void
      {
         super.handleMouseRelease(event);
         ExternalInterface.call("sound_play",UiSound.CLICK);
      }
      
      override protected function handleMouseRollOut(event:MouseEvent) : void
      {
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var index:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         if(event.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OUT));
            if(Boolean(_mouseDown & 1 << index))
            {
               if(stage != null)
               {
                  stage.addEventListener(MouseEvent.MOUSE_UP,this.handleReleaseOutside,false,0,true);
               }
            }
            if(lockDragStateChange || !enabled)
            {
               return;
            }
            if(!_isRepeating || _state == "over")
            {
               this.setState("out");
            }
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
         ExternalInterface.call("sound_play",UiSound.HOVER);
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
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
            this.setState("over");
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

