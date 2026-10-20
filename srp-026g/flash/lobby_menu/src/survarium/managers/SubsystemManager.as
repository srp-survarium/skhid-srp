package survarium.managers
{
   import flash.display.DisplayObject;
   import flash.utils.Dictionary;
   import survarium.interfaces.ISubSystem;
   
   public class SubsystemManager
   {
      
      private static var _subsystems:Dictionary = new Dictionary();
      
      public function SubsystemManager()
      {
         super();
      }
      
      public static function addSubsystem(value:String, renderer:DisplayObject) : void
      {
         trace("addSubsystem",value,renderer);
         _subsystems[value] = renderer;
      }
      
      public static function setSybsystemEnabled(subsystem:String, value:Boolean) : void
      {
         trace("setSybsystemEnabled",_subsystems[subsystem],_subsystems[subsystem] as ISubSystem);
         (_subsystems[subsystem] as ISubSystem).subsystem_enable = value;
      }
      
      public static function setSybsystemActive(subsystem:String, value:Boolean) : void
      {
         trace("setSybsystemActive",_subsystems[subsystem],_subsystems[subsystem] as ISubSystem);
         (_subsystems[subsystem] as ISubSystem).subsystem_active = value;
      }
   }
}

