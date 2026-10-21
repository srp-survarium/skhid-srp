package survarium.controls
{
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.Events;
   import survarium.data.LocalizationData;
   import survarium.interfaces.ISubSystem;
   
   public class UIWindow extends UIComponent implements ISubSystem
   {
      
      public var btnClose:Button;
      
      public var btnClose2:Button;
      
      private var _subsystemName:String;
      
      public function UIWindow()
      {
         super();
         if(this.btnClose)
         {
            this.btnClose.addEventListener(ButtonEvent.CLICK,this.closeWindow);
         }
         if(this.btnClose2)
         {
            LocalizationData.addButton(this.btnClose2,"close");
         }
         if(this.btnClose2)
         {
            this.btnClose2.addEventListener(ButtonEvent.CLICK,this.closeWindow);
         }
      }
      
      public function get subsystem_name() : String
      {
         return this._subsystemName;
      }
      
      public function set subsystem_name(value:String) : void
      {
         _subsystemName = value;
      }
      
      public function activate(show:Boolean, byUser:Boolean = true) : void
      {
         trace("activate",name,show,byUser);
         if(byUser)
         {
            if(!visible && show)
            {
               trace("subsystem_activated",this._subsystemName);
               ExternalInterface.call("subsystem_activated",this._subsystemName);
            }
            else if(visible && !show)
            {
               trace("subsystem_deactivated",this._subsystemName);
               ExternalInterface.call("subsystem_deactivated",this._subsystemName);
            }
         }
         visible = show;
      }
      
      public function get subsystem_enable() : Boolean
      {
         return enabled;
      }
      
      public function set subsystem_enable(value:Boolean) : void
      {
         trace("subsystem_enable",this,value,enabled);
         if(value && !enabled)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ENABLED));
         }
         else if(!value && enabled)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DISABLED));
         }
         enabled = value;
      }
      
      public function get subsystem_active() : Boolean
      {
         return visible && parent;
      }
      
      public function set subsystem_active(value:Boolean) : void
      {
         if(!value)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DEACTIVATED));
         }
         else
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ACTIVATED));
         }
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function closeWindow(event:Event = null) : void
      {
         MainScene.activeWindow = null;
         visible = false;
         dispatchEvent(new Event(Event.CLOSE));
      }
   }
}

