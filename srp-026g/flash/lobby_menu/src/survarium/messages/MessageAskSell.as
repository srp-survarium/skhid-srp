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
      
      private var _warned:Boolean = false;
      
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
      
      // The data comes before Message centres the dialog (and starts its
      // tween): the warning's room is made here, so the dialog is centred
      // at its final height.
      override public function setMessageData(id:int, messageData:Object) : void
      {
         super.setMessageData(id,messageData);
         this.warnPresets();
      }
      
      // An item a kit preset uses (presets.md, the owner's of 2026-10-23):
      // the dialog asks first, a line under the item's name ("Used in
      // Loadout 2 and Loadout 4. Sell anyway?", wrapped centred when long).
      // The rows under the name (the slider, the cost, the buttons) move
      // down by the warning's height and a gap, and the warning sits centred
      // between the name and the first of them. Cancel sells nothing; the
      // server sells as ever. An item no preset uses: the stock dialog.
      private function warnPresets() : void
      {
         var text:String = _data != null && !this._warned ? LocalPlayerProfile.sellWarning(uint(_data.id)) : "";
         if(text == "")
         {
            return;
         }
         this._warned = true;
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
         warning.width = background.width - 20;
         warning.text = text;
         var high:Number = Math.ceil(warning.textHeight) + 4;
         warning.height = high;
         // The name's line (configUI sets the same text again); the rows
         // under it start at the slider's place (the cost row, of an item
         // without a slider, is at the slider's).
         _ItemLabel.text = String(_data.name);
         var below:Number = _ItemLabel.y + 18;
         var nameEnd:Number = _ItemLabel.y + Math.ceil(_ItemLabel.textHeight) + 2;
         var top:Number = slider.y;
         var room:Number = high + 6;
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
         warning.y = Math.round(nameEnd + (top + room - nameEnd - high) / 2);
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

