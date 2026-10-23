package survarium.messages
{
   import flash.display.DisplayObject;
   import flash.external.ExternalInterface;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import survarium.constants.Colors;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;
   import survarium.player.LocalPlayerProfile;
   
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
         this.warnPresets();
      }
      
      // An item a kit preset uses (presets.md, the owner's of 2026-10-23):
      // the dialog asks first, a line under the item's name ("Used in
      // Loadout 2 and Loadout 4. Sell anyway?"); the rest moves down. Cancel
      // sells nothing; the server sells as ever.
      private function warnPresets() : void
      {
         var text:String = _data != null ? LocalPlayerProfile.sellWarning(uint(_data.id)) : "";
         if(text == "")
         {
            return;
         }
         var room:Number = 40;
         var below:Number = _ItemLabel.y + 18;
         var i:int = 0;
         while(i < numChildren)
         {
            var child:DisplayObject = getChildAt(i);
            if(child != background && child != content && child.y >= below)
            {
               child.y += room;
            }
            i++;
         }
         background.height += room;
         var warning:TextField = new TextField();
         var format:TextFormat = new TextFormat("Calibri",14,Colors.ORANGE,true);
         format.align = TextFormatAlign.CENTER;
         warning.defaultTextFormat = format;
         warning.multiline = true;
         warning.wordWrap = true;
         warning.selectable = false;
         warning.mouseEnabled = false;
         warning.filters = [new DropShadowFilter(2,45,0,1,3,3)];
         warning.x = 10;
         warning.y = below + 2;
         warning.width = background.width - 20;
         warning.height = room;
         warning.text = text;
         addChild(warning);
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

