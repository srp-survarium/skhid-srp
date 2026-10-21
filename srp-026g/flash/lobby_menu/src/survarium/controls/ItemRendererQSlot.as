package survarium.controls
{
   import flash.text.TextField;
   import survarium.Util;
   import survarium.ammo.AmmoSlider;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.constants.ItemPlace;
   import survarium.constants.QuickSlotSize;
   
   public class ItemRendererQSlot extends ItemRendererBase
   {
      
      public var textCount:TextField;
      
      public var slider:AmmoSlider;
      
      public var pocketSize:int = 5;
      
      public function ItemRendererQSlot()
      {
         super();
      }
      
      override protected function updateData() : void
      {
         var item:ItemData;
         var isEquipped:Boolean;
         item = _data as ItemData;
         super.updateData();
         this.slider.renderer = "PDSliderCell";
         isEquipped = item.descriptor != ItemPlace.NONE;
         var capacity:int = isEquipped ? PlayerInfo.pocketSize : this.pocketSize;
         var totalCount:int = isEquipped ? item.count : item.value;
         this.slider.visible = true;
         this.slider.scaleRenders = false;
         this.slider.shiftInterval = 1;
         this.slider.minimum = 0;
         var isDouble:Boolean = _data.quickSlotItemSize == QuickSlotSize.WHOLE;
         trace("IRQ::",item.name,item.descriptor,item.count,item.value,isDouble);
         this.textCount.visible = this.slider.visible = !isDouble;
         var slotSize:int = isDouble ? capacity : 1;
         this.slider.maximum = slotSize * 5;
         this.slider.snapInterval = slotSize;
         this.slider.available = capacity;
         this.slider.total = Math.min(totalCount * slotSize,int(capacity / slotSize) * slotSize);
         var loadedCount:int = Math.min(_data.value,totalCount) * slotSize;
         this.slider.value = loadedCount;
         this.textCount.text = "x" + loadedCount.toString();
         item.tipText = Util.styledChar(LocalizationData.getName("backpack") + ": ","gray") + Util.styledChar(item.value.toString(),"yellow") + Util.styledChar(", ","gray") + Util.styledChar(LocalizationData.getName("storage") + ": ","gray") + Util.styledChar((totalCount - item.value).toString(),"yellow");
         trace("SLIDER_INFO",totalCount,this.slider.value,this.slider.maximum,this.slider.total,this.slider.available,this.slider.snapInterval);
         this.slider.mouseEnabled = isEquipped;
         this.slider.mouseChildren = isEquipped;
         this.slider.invalidateSettings();
      }
   }
}

