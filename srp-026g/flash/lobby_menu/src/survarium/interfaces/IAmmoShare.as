package survarium.interfaces
{
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface IAmmoShare extends IUIComponent
   {
      
      function set updateFunction(value:Function) : void;
      
      function set autoBuyCheckBox(value:Button) : void;
      
      function set acceptButton(value:Button) : void;
      
      function set cancelButton(value:Button) : void;
      
      function set ammoBags(value:uint) : void;
      
      function set pocketSize(value:uint) : void;
      
      function setAmmo(event:ButtonEvent = null) : void;
      
      function swapElements(firstIndex:int, secondIndex:int) : void;
      
      function registerWeightTextField(weightField:TextField) : void;
   }
}

