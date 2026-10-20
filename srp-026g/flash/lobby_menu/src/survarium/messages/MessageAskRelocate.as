package survarium.messages
{
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;
   
   public class MessageAskRelocate extends MessageAsk
   {
      
      public function MessageAskRelocate()
      {
         super();
         _CountPrice.visible = false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addButton(btnAccept,"relocate");
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         Message.onClick(_id);
         if(slider.visible)
         {
            _data.count = slider.value;
         }
         else
         {
            _data.count = 1;
         }
         var count:uint = _data.count;
         trace("relocate_ok_clicked",_data.profile,_data.id,_data.dict_id,_data.sourceSlot,_data.targetSlot,count);
         var _relocateArray:Array = [{
            profile:_data.profile,
            id:_data.id,
            dict_id:_data.dict_id,
            sourceSlot:_data.sourceSlot,
            targetSlot:_data.targetSlot,
            count:count
         }];
         _data.onOK(_relocateArray);
         _data.onOK = null;
         _data = null;
      }
   }
}

