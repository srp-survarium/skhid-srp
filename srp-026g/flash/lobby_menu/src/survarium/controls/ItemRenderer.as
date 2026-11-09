package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ToolTip;
   import survarium.constants.Colors;
   import survarium.constants.ItemCategory;
   import survarium.constants.SuperCategory;
   
   public class ItemRenderer extends ToolTip
   {
      
      protected var _countTextFormat:TextFormat;
      
      public var iconRenderer:IconRenderer;
      
      public var textCount:TextField;
      
      public var damageIndicator:MovieClip;
      
      public function ItemRenderer()
      {
         super();
      }
      
      public function set data(value:Object) : void
      {
         _data = value;
         invalidateData();
      }
      
      public function get data() : Object
      {
         return _data;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         _countTextFormat = this.textCount.getTextFormat();
         this.iconRenderer.center = true;
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(_data)
            {
               visible = true;
               this.updateData();
            }
            else
            {
               visible = false;
            }
         }
      }
      
      protected function updateData() : void
      {
         var isMoney:Boolean;
         this.iconRenderer.content = _data.icon;
         // No damage bar: items never wear (no repair, the owner 2026-11-08).
         this.damageIndicator.visible = false;
         this.iconRenderer.y = _data.category == ItemCategory.DECALS ? -5 : -10;
         if(!_data.isStack)
         {
            this.textCount.visible = false;
         }
         else
         {
            this.textCount.visible = true;
            this._countTextFormat.align = _data.superCategory == SuperCategory.UPGRADES ? TextFormatAlign.CENTER : TextFormatAlign.RIGHT;
            this.textCount.defaultTextFormat = this._countTextFormat;
            isMoney = _data.superCategory == SuperCategory.UPGRADES;
            this.textCount.text = (isMoney ? "" : "x") + _data.value;
            this.textCount.y = 42 + (isMoney ? 0 : 5);
            this.textCount.textColor = isMoney ? Colors.REGULAR : Colors.SILVER;
         }
      }
   }
}

