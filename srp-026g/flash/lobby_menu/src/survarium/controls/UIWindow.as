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
      
      public function set subsystem_name(param1:String) : void
      {
         _subsystemName = param1;
      }
      
      public function activate(param1:Boolean, param2:Boolean = true) : void
      {
         trace("activate",name,param1,param2);
         if(param2)
         {
            if(!visible && param1)
            {
               trace("subsystem_activated",this._subsystemName);
               ExternalInterface.call("subsystem_activated",this._subsystemName);
            }
            else if(visible && !param1)
            {
               trace("subsystem_deactivated",this._subsystemName);
               ExternalInterface.call("subsystem_deactivated",this._subsystemName);
            }
         }
         visible = param1;
      }
      
      public function get subsystem_enable() : Boolean
      {
         return enabled;
      }
      
      public function set subsystem_enable(param1:Boolean) : void
      {
         trace("subsystem_enable",this,param1,enabled);
         if(param1 && !enabled)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ENABLED));
         }
         else if(!param1 && enabled)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DISABLED));
         }
         enabled = param1;
      }
      
      public function get subsystem_active() : Boolean
      {
         return visible && parent;
      }
      
      public function set subsystem_active(param1:Boolean) : void
      {
         if(!param1)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DEACTIVATED));
         }
         else
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ACTIVATED));
         }
      }
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(param1:Number) : void
      {
         super.y = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function closeWindow(param1:Event = null) : void
      {
         MainScene.activeWindow = null;
         visible = false;
         dispatchEvent(new Event(Event.CLOSE));
      }
   }
}

