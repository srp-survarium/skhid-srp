package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;
   
   public class MessageFactionRank extends MessageAccept
   {
      
      public var factionIcon:MovieClip;
      
      public var textFieldLabel:TextField;
      
      public var textFieldUnlocked:TextField;
      
      public var itemsTileList:TileList;
      
      public var btnPrev:Button;
      
      public var btnNext:Button;
      
      public function MessageFactionRank()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         var newItems:Array;
         super.configUI();
         btnAccept.label = LocalizationData.getName("goto_shop");
         btnCancel.label = LocalizationData.getName("close");
         this.btnPrev.addEventListener(ButtonEvent.CLICK,this.prevItem);
         this.btnNext.addEventListener(ButtonEvent.CLICK,this.nextItem);
         this.itemsTileList.addEventListener(ListEvent.INDEX_CHANGE,this.onListIndexChange);
         if(_data)
         {
            this.factionIcon.gotoAndStop(_data.faction_id);
            newItems = _data.items;
            trace("MessageFactionRank::",newItems.length);
            this.itemsTileList.dataProvider = new DataProvider(newItems);
            this.itemsTileList.selectedIndex = 0;
            this.textFieldUnlocked.text = String(newItems.length) + " " + LocalizationData.getName("new_items_unlocked");
            this.textFieldLabel.text = _data.text;
         }
      }
      
      private function onListIndexChange(e:ListEvent) : void
      {
         if(_data)
         {
            _data.dictId = e.itemData.dictId;
         }
      }
      
      private function nextItem(e:ButtonEvent) : void
      {
         if(this.itemsTileList.selectedIndex != this.itemsTileList.dataProvider.length - 1)
         {
            this.itemsTileList.selectedIndex += 1;
         }
      }
      
      private function prevItem(e:ButtonEvent) : void
      {
         if(this.itemsTileList.selectedIndex != 0)
         {
            this.itemsTileList.selectedIndex -= 1;
         }
      }
      
      override protected function draw() : void
      {
         super.draw();
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         if(_data)
         {
            if(_data.onNo is Function)
            {
               _data.onNo();
            }
            _data.onNo = null;
            _data.onYes = null;
         }
         Message.onClick(_id,"no");
         ExternalInterface.call("cancel_changes",_data.selectedIndex);
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         if(_data)
         {
            if(_data.onYes is Function)
            {
               _data.onYes();
            }
            _data.onNo = null;
            _data.onYes = null;
         }
         dispatchEvent(new Event(Event.OPEN));
         Message.onClick(_id,"yes",_data);
      }
   }
}

