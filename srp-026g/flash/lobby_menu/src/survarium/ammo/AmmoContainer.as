package survarium.ammo
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import survarium.data.LocalizationData;
   
   public class AmmoContainer extends UIComponent
   {
      
      public var tx_total_label:TextField;
      
      public var btn_apply:Button;
      
      public var btn_cancel:Button;
      
      public var maskSprite:Sprite;
      
      protected var start_y:Number = 0;
      
      protected var _shift:int = 0;
      
      public function AmmoContainer()
      {
         super();
         start_y = y;
         mask = this.maskSprite;
         addEventListener(FocusEvent.FOCUS_IN,this.onFocusIn);
         addEventListener(FocusEvent.FOCUS_OUT,this.onFocusIn);
         LocalizationData.addTextField(this.tx_total_label,"ff_ammo_total_cost_label");
      }
      
      private function onFocusIn(event:FocusEvent) : void
      {
         if(event.type == FocusEvent.FOCUS_IN)
         {
            MainScene.activePop = this;
         }
         else if(MainScene.activePop == this)
         {
            MainScene.activePop = null;
         }
      }
      
      public function closePopup() : void
      {
         dispatchEvent(new Event(Event.CLOSE,true));
      }
      
      public function set shift(value:int) : void
      {
         _shift = value;
         y = this.start_y + value;
      }
      
      public function get shift() : int
      {
         return this._shift;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = value;
         this.maskSprite.y = -(value - this.start_y);
      }
   }
}

