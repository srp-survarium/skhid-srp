package survarium.controls
{
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import survarium.ammo.AmmoSlider;
   import survarium.data.ItemData;
   import survarium.constants.Colors;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemPlace;
   import survarium.constants.QuickSlotSize;
   
   public class ItemRendererQSlot extends ItemRendererBase
   {
      
      public var textCount:TextField;
      
      public var slider:AmmoSlider;
      
      public var pocketSize:int = 5;

      // The pocket's cells are its units (docs/notes/one-pocket-a-type.md, "The pocket's units
      // shown"): an item a cell, an RGD-5 too; the units its pairs use up last, in the speed
      // warning's pale red (WeightEffects.WARNING_COLOR); the unit Additional Medicine adds in pale green.
      private static const BLOCKED_TINT:ColorTransform = tint(0xF18A78);

      private static const BONUS_TINT:ColorTransform = tint(Colors.GREEN);

      private static const BONUS_EMPTY_TINT:ColorTransform = tint(Colors.GREEN,0.45);

      private static function tint(colour:uint, alpha:Number = 1) : ColorTransform
      {
         return new ColorTransform(0,0,0,alpha,colour >> 16 & 0xFF,colour >> 8 & 0xFF,colour & 0xFF,0);
      }
      
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
         var base:int = isEquipped ? PlayerInfo.pocketSize : this.pocketSize;
         var capacity:int = PlayerInfo.pocketUnits(item.category,item.quickSlotItemSize,base);
         // the pocket's units
         var units:int = PlayerInfo.pocketTotal(item.category,base);
         var totalCount:int = isEquipped ? item.count : item.value;
         this.slider.visible = true;
         this.slider.scaleRenders = false;
         this.slider.shiftInterval = 1;
         this.slider.minimum = 0;
         var isDouble:Boolean = _data.quickSlotItemSize == QuickSlotSize.WHOLE && capacity == 1;
         trace("IRQ::",item.name,item.descriptor,item.count,item.value,isDouble);
         this.textCount.visible = this.slider.visible = !isDouble;
         var slotSize:int = 1;
         this.slider.openCells = units;
         this.slider.blockedCells = units - capacity;
         this.slider.blockedTint = BLOCKED_TINT;
         // Green: Additional Medicine's extra medkit unit, the pocket's last.
         this.slider.bonusCell = PlayerInfo.medicineBonus(item.category) ? units - 1 : -1;
         this.slider.bonusTint = BONUS_TINT;
         this.slider.bonusEmptyTint = BONUS_EMPTY_TINT;
         this.slider.maximum = Math.max(5,units);
         this.slider.snapInterval = slotSize;
         this.slider.available = capacity;
         this.slider.total = Math.min(totalCount * slotSize,int(capacity / slotSize) * slotSize);
         var loadedCount:int = Math.min(_data.value,totalCount) * slotSize;
         this.slider.value = loadedCount;
         this.textCount.text = "x" + loadedCount.toString();
         trace("SLIDER_INFO",totalCount,this.slider.value,this.slider.maximum,this.slider.total,this.slider.available,this.slider.snapInterval);
         this.slider.mouseEnabled = isEquipped;
         this.slider.mouseChildren = isEquipped;
         this.slider.invalidateSettings();
      }
   }
}

