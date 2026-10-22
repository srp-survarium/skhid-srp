package survarium.interfaces
{
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface ISkillRenderer extends IUIComponent
   {
      
      function get data() : Object;
      
      function set data(value:Object) : void;
      
      function set unlockable(value:Boolean) : void;
      
      function showLock(show:Boolean) : void;
      
      function lock(locked:Boolean) : void;
      
      function invalidateData() : void;
   }
}

