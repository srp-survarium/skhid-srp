package survarium.interfaces
{
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface ISkillRenderer extends IUIComponent
   {
      
      function get data() : Object;
      
      function set data(param1:Object) : void;
      
      function set unlockable(param1:Boolean) : void;
      
      function showLock(param1:Boolean) : void;
      
      function lock(param1:Boolean) : void;
      
      function invalidateData() : void;
   }
}

