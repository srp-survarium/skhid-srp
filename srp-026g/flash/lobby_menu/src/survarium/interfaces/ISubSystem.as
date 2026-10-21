package survarium.interfaces
{
   public interface ISubSystem
   {
      
      function get subsystem_name() : String;
      
      function set subsystem_name(value:String) : void;
      
      function get subsystem_enable() : Boolean;
      
      function set subsystem_enable(value:Boolean) : void;
      
      function get subsystem_active() : Boolean;
      
      function set subsystem_active(value:Boolean) : void;
   }
}

