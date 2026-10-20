package survarium.messages
{
   import flash.text.TextField;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.NumericStepperExchange;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   
   public class MessageMoneyExchange extends MessageAccept
   {
      
      public var textFieldLabel:TextField;
      
      public var exchangeStepper:NumericStepperExchange;
      
      public function MessageMoneyExchange()
      {
         super();
         SrpExchange.build(this);
      }
      
      override protected function configUI() : void
      {
         var _loc1_:Boolean;
         super.configUI();
         this.exchangeStepper.setExchangeMoneyType(2);
         this.exchangeStepper.setExchangeMoneyFactor(PlayerInfo.premiumExchangeRate);
         _loc1_ = PlayerInfo.premiumMoney > 0;
         this.exchangeStepper.minimum = _loc1_ ? 1 : 0;
         this.exchangeStepper.value = this.exchangeStepper.minimum;
         this.exchangeStepper.maximum = Math.min(uint.MAX_VALUE,PlayerInfo.premiumMoney);
         this.exchangeStepper.editable = true;
         this.exchangeStepper.stepSize = 1;
         this.exchangeStepper.enabled = _loc1_;
         btnAccept.label = LocalizationData.getName("exchange_button");
         btnAccept.enabled = _loc1_;
         this.textFieldLabel.text = _data.label;
         SrpExchange.apply(this);
      }
      
      override protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         SrpExchange.accept(this);
      }
   }
}

