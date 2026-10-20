package survarium.interfaces
{
   public interface ISubSystem
   {
      
      function get subsystem_name() : String;
      
      function set subsystem_name(param1:String) : void;
      
      function get subsystem_enable() : Boolean;
      
      function set subsystem_enable(param1:Boolean) : void;
      
      function get subsystem_active() : Boolean;
      
      function set subsystem_active(param1:Boolean) : void;
   }
}

