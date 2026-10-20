package survarium.buttons
{
   import flash.display.Sprite;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import scaleform.clik.controls.StatusIndicator;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.utils.Constraints;
   import scaleform.gfx.MouseEventEx;
   import survarium.Util;
   
   public class ButtonQuestFaction extends ButtonIconCenter
   {
      
      public var progressIndicator:StatusIndicator;
      
      public var selectedIcon:Sprite;
      
      private var _active:Boolean = false;
      
      protected var statesActive:Vector.<String> = Vector.<String>(["active_",""]);
      
      public function ButtonQuestFaction()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraintsDisabled && constraints)
         {
            constraints.addElement("selectedIcon",this.selectedIcon,Constraints.RIGHT);
            constraints.addElement("progressIndicator",this.progressIndicator,Constraints.LEFT | Constraints.RIGHT);
         }
      }
      
      override public function set data(param1:Object) : void
      {
         if(param1 && param1.faction_enabled != undefined)
         {
            enabled = param1.faction_enabled;
         }
         _active = param1 && param1.selected;
         super.data = param1;
         invalidateData();
         setState(state);
         invalidateState();
      }
      
      override protected function handleMouseRelease(param1:MouseEvent) : void
      {
         _autoRepeatEvent = null;
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         var _loc4_:uint = _loc2_ == null ? 0 : _loc2_.buttonIdx;
         var _loc5_:ButtonEvent;
         if(_loc4_ != 0)
         {
            return;
         }
         _mouseDown ^= 1 << _loc3_;
         if(_mouseDown == 0 && _repeatTimer)
         {
            _repeatTimer.stop();
            _repeatTimer.reset();
            _repeatTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,beginRepeat);
            _repeatTimer.removeEventListener(TimerEvent.TIMER,handleRepeat);
            _repeatTimer = null;
         }
         setState("release");
         handleClick(_loc3_);
         if(!_isRepeating)
         {
            _loc5_ = new ButtonEvent(ButtonEvent.CLICK,true,false,_loc3_,_loc4_,false,false);
            dispatchEvent(_loc5_);
         }
         _isRepeating = false;
      }
      
      override protected function updateAfterStateChange() : void
      {
         if(!initialized)
         {
            return;
         }
         if(constraints != null && !constraintsDisabled)
         {
            constraints.updateElement("progressIndicator",this.progressIndicator);
            constraints.updateElement("selectedIcon",this.selectedIcon);
         }
         super.updateAfterStateChange();
      }
      
      override protected function getStatePrefixes() : Vector.<String>
      {
         return super.getStatePrefixes();
      }
      
      override protected function updateText() : void
      {
         trace("BQF::",icon,_data,this.progressIndicator);
         this.selectedIcon.visible = this._active;
         if(_data)
         {
            _data.icon = _data.faction_id;
            _data.name = Util.styledChar(_data.faction_name,"white") + "\n" + _data.faction_label + Util.styledChar("fff\n\n\n","small") + _data.faction_hint;
            if(this.progressIndicator)
            {
               this.progressIndicator.maximum = 100;
               this.progressIndicator.value = _data.faction_progress;
               this.progressIndicator.validateNow();
            }
         }
         super.updateText();
         if(icon && _data)
         {
            if(selected || this._active || state == "over")
            {
               icon.alpha = enabled ? 1 : 0.6;
               this.progressIndicator.alpha = icon.alpha;
               this.selectedIcon.alpha = selected ? 1 : 0.8;
            }
            else
            {
               icon.alpha = enabled == true ? 0.8 : 0.3;
               this.progressIndicator.alpha = icon.alpha;
               this.selectedIcon.alpha = icon.alpha;
            }
         }
      }
   }
}

