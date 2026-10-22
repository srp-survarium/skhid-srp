package survarium.controls
{
   import flash.display.MovieClip;
   
   public class NumericStepperExchange extends NumericStepperLabel
   {
      
      protected var _exchangeType:uint;
      
      protected var _exchangeFactor:uint;
      
      public var iconMoney:MovieClip;
      
      public function NumericStepperExchange()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.iconMoney.gotoAndStop(1);
      }
      
      public function setExchangeMoneyFactor(factor:uint) : void
      {
         _exchangeFactor = factor;
         invalidateData();
      }
      
      public function setExchangeMoneyType(moneyType:uint) : void
      {
         _exchangeType = moneyType;
         invalidateData();
      }
      
      override protected function updateLabel() : void
      {
         super.updateLabel();
         if(this.iconMoney && this.iconMoney.currentFrame != this._exchangeType)
         {
            this.iconMoney.gotoAndStop(this._exchangeType);
         }
         if(textFieldLabel)
         {
            textFieldLabel.text = String(_value * this._exchangeFactor);
         }
      }
   }
}

