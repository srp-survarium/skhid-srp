package survarium.buttons
{
   import flash.text.TextField;
   import flash.utils.clearInterval;
   import flash.utils.setInterval;
   import survarium.Util;
   import survarium.controls.FixedText;
   import survarium.data.LocalizationData;
   
   public class ButtonPremium extends ButtonClick
   {
      
      public var textField2:FixedText;
      
      public var textField3:TextField;
      
      protected var _stateIndex:uint = 0;
      
      protected var _discount:int = 0;
      
      private var displayTime:Boolean = false;
      
      private var displayTimeInterval:int = -1;
      
      protected var statesPremiumAvailable:Vector.<String> = Vector.<String>(["premium_"]);
      
      protected var statesPremiumWarning:Vector.<String> = Vector.<String>(["warning_"]);
      
      protected var statesPremiumDiscount:Vector.<String> = Vector.<String>(["discount_"]);
      
      public function ButtonPremium()
      {
         super();
         LocalizationData.addCallback(this.onLocal,"premium");
      }
      
      public function set stateIndex(value:uint) : void
      {
         _stateIndex = value;
         setState(_state);
         invalidateState();
      }
      
      public function set discount(value:int) : void
      {
         _discount = Math.max(value,0);
         setState(_state);
         invalidateState();
         if(this.discount > 0)
         {
            if(this.displayTimeInterval == -1)
            {
               displayTimeInterval = setInterval(this.toggleTime,4000);
            }
         }
         else if(this.displayTimeInterval != -1)
         {
            clearInterval(this.displayTimeInterval);
            displayTimeInterval = -1;
         }
      }
      
      private function toggleTime() : void
      {
         displayTime = !this.displayTime;
         this.updateText();
      }
      
      public function get discount() : int
      {
         return this._discount;
      }
      
      protected function onLocal(value:String = "") : void
      {
         this.updateText();
      }
      
      override protected function updateText() : void
      {
         var str:String;
         if(textField)
         {
            if(this._discount)
            {
               str = this.displayTime ? Util.getTimeString(this._discount) : LocalizationData.getName("premium_action").toUpperCase();
            }
            else
            {
               str = this._stateIndex != 0 ? _label : LocalizationData.getName("premium").toUpperCase();
            }
            textField.text = str;
         }
         trace("updatePremium::",str,"|",_label);
      }
      
      override protected function getStatePrefixes() : Vector.<String>
      {
         if(this._discount)
         {
            return this.statesPremiumDiscount;
         }
         if(this._stateIndex == 0)
         {
            return statesDefault;
         }
         if(this._stateIndex == 1)
         {
            return this.statesPremiumAvailable;
         }
         return this.statesPremiumWarning;
      }
   }
}

