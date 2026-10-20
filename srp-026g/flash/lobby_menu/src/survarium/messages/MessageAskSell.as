package survarium.messages
{
   import flash.external.ExternalInterface;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;
   
   public class MessageAskSell extends MessageAsk
   {
      
      public function MessageAskSell()
      {
         super();
      }
      
      override protected function onSliderUpdate(event:IndexEvent = null) : void
      {
         var pos_2:Number = Math.min(slider.value,_data.count);
         if(pos_2 > slider.maximum)
         {
            pos_2 = slider.maximum;
         }
         var sliderpos:int = pos_2;
         var ammount:int = Math.ceil(sliderpos / _step * _price);
         trace("onSliderUpdate",sliderpos,ammount,_step,pos_2,_price,"|",slider.value,_data.count);
         _CountPrice.cost = String(int(ammount));
         var toAdd:String = "";
         if(pos_2 - sliderpos > 0)
         {
            toAdd = " + " + int(pos_2 * _step - sliderpos * _step) + LocalizationData.getName("item_piece_postfix");
         }
         if(_step > 1)
         {
            _CountLabel.text = "x" + _step.toString();
            _CountLabel2.text = toAdd;
         }
         else
         {
            _CountLabel.text = sliderpos.toString() + LocalizationData.getName("item_piece_postfix");
         }
         if(pos_2 == 0)
         {
            btnAccept.enabled = false;
         }
         else
         {
            btnAccept.enabled = true;
         }
         if(slider.visible)
         {
            updateCostPos();
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addTextField(_windowLabel,"sell");
         btnAccept.label = LocalizationData.getName("sell").toUpperCase();
         _CountLabel2.visible = false;
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         Message.onClick(_id);
         var pos_2:Number = Math.min(slider.value,_data.count);
         if(pos_2 > slider.maximum)
         {
            pos_2 = slider.maximum;
         }
         if(slider.visible)
         {
            _data.count = Math.ceil(pos_2 / _step);
         }
         else
         {
            _data.count = 1;
         }
         trace("relocate_ok_clicked",_data.id,_data.count,_step,uint(_data.count));
         ExternalInterface.call("sell_ok_clicked",_data.id,uint(_data.count));
      }
   }
}

