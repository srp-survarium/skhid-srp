package survarium.controls
{
   import flash.text.TextField;
   import survarium.Util;
   import survarium.ammo.AmmoSlider;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   
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
         var _loc1_:ItemData;
         var _loc2_:Boolean;
         _loc1_ = _data as ItemData;
         super.updateData();
         this.slider.renderer = "PDSliderCell";
         _loc2_ = _loc1_.descriptor != 0;
         var _loc3_:int = _loc2_ ? PlayerInfo.pocketSize : this.pocketSize;
         var _loc4_:int = _loc2_ ? _loc1_.count : _loc1_.value;
         this.slider.visible = true;
         this.slider.scaleRenders = false;
         this.slider.shiftInterval = 1;
         this.slider.minimum = 0;
         var _loc5_:Boolean = _data.quickSlotItemSize == 2;
         trace("IRQ::",_loc1_.name,_loc1_.descriptor,_loc1_.count,_loc1_.value,_loc5_);
         this.textCount.visible = this.slider.visible = !_loc5_;
         var _loc6_:int = _loc5_ ? _loc3_ : 1;
         this.slider.maximum = _loc6_ * 5;
         this.slider.snapInterval = _loc6_;
         this.slider.available = _loc3_;
         this.slider.total = Math.min(_loc4_ * _loc6_,int(_loc3_ / _loc6_) * _loc6_);
         var _loc7_:int = Math.min(_data.value,_loc4_) * _loc6_;
         this.slider.value = _loc7_;
         this.textCount.text = "x" + _loc7_.toString();
         _loc1_.tipText = Util.styledChar(LocalizationData.getName("backpack") + ": ","gray") + Util.styledChar(_loc1_.value.toString(),"yellow") + Util.styledChar(", ","gray") + Util.styledChar(LocalizationData.getName("storage") + ": ","gray") + Util.styledChar((_loc4_ - _loc1_.value).toString(),"yellow");
         trace("SLIDER_INFO",_loc4_,this.slider.value,this.slider.maximum,this.slider.total,this.slider.available,this.slider.snapInterval);
         this.slider.mouseEnabled = _loc2_;
         this.slider.mouseChildren = _loc2_;
         this.slider.invalidateSettings();
      }
   }
}

