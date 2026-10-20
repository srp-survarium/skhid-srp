package survarium.interfaces
{
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface IAmmoShare extends IUIComponent
   {
      
      function set updateFunction(param1:Function) : void;
      
      function set autoBuyCheckBox(param1:Button) : void;
      
      function set acceptButton(param1:Button) : void;
      
      function set cancelButton(param1:Button) : void;
      
      function set ammoBags(param1:uint) : void;
      
      function set pocketSize(param1:uint) : void;
      
      function setAmmo(param1:ButtonEvent = null) : void;
      
      function swapElements(param1:int, param2:int) : void;
      
      function registerWeightTextField(param1:TextField) : void;
   }
}

