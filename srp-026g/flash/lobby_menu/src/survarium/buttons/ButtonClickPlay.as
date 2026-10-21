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
      
      public function set subsystem_name(value:String) : void
      {
         _subsystemName = value;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         if(value)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ENABLED));
         }
         else
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DISABLED));
         }
         super.enabled = value;
      }
      
      override public function set visible(value:Boolean) : void
      {
         if(!value)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DEACTIVATED));
         }
         super.visible = value;
      }
      
      public function get subsystem_enable() : Boolean
      {
         return enabled;
      }
      
      public function set subsystem_enable(value:Boolean) : void
      {
         enabled = value;
      }
      
      public function get subsystem_active() : Boolean
      {
         return visible && parent;
      }
      
      public function set subsystem_active(value:Boolean) : void
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
      
      override protected function handleMouseRelease(event:MouseEvent) : void
      {
         super.handleMouseRelease(event);
         ExternalInterface.call("sound_play",3);
      }
      
      override protected function setState(state:String) : void
      {
         if(_data && _data is Object)
         {
            _state = state;
         }
         if(state == "down")
         {
            ExternalInterface.call("sound_play",2);
         }
         var prefixes:Vector.<String> = getStatePrefixes();
         var states:Array = _stateMap[state];
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
      
      override protected function handleMouseRollOut(event:MouseEvent) : void
      {
         var eventEx:MouseEventEx = event as MouseEventEx;
         var mouseIndex:uint = eventEx == null ? 0 : eventEx.mouseIdx;
         _mouseOver = false;
         if(event.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OUT));
            if(Boolean(_mouseDown & 1 << mouseIndex))
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
         ExternalInterface.call("sound_play",1);
         var eventEx:MouseEventEx = event as MouseEventEx;
         var mouseIndex:uint = eventEx == null ? 0 : eventEx.mouseIdx;
         _mouseOver = true;
         if(event.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OVER));
            if(!enabled)
            {
               return;
            }
            if(lockDragStateChange && Boolean(_mouseDown << mouseIndex & 1))
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

