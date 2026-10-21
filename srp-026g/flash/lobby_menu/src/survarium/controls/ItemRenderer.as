package survarium.controls
{
   import flash.display.MovieClip;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ToolTip;
   import survarium.Util;
   import survarium.constants.Colors;
   
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
      
      public function set data(param1:Object) : void
      {
         _data = param1;
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
         var _loc1_:int;
         var _loc2_:uint;
         var _loc3_:ColorTransform;
         var _loc4_:Boolean;
         this.iconRenderer.content = _data.icon;
         this.damageIndicator.visible = !_data.isStack && _data.value < 100;
         trace("IR::updateData",_data.name,_data.isStack,_data.value,this.damageIndicator.visible);
         this.iconRenderer.y = _data.category == 26 ? -5 : -10;
         if(!_data.isStack)
         {
            this.textCount.visible = false;
            this.damageIndicator.track.width = 165 * _data.value / 100;
            _loc1_ = _data.value / 100 * 510;
            _loc2_ = Util.getColor(Math.min(510 - _loc1_,255),Math.min(_loc1_,255),0);
            _loc3_ = new ColorTransform();
            _loc3_.color = _loc2_;
            this.damageIndicator.track.transform.colorTransform = _loc3_;
         }
         else
         {
            this.textCount.visible = true;
            this._countTextFormat.align = _data.superCategory == 5 ? TextFormatAlign.CENTER : TextFormatAlign.RIGHT;
            this.textCount.defaultTextFormat = this._countTextFormat;
            _loc4_ = _data.superCategory == 5;
            this.textCount.text = (_loc4_ ? "" : "x") + _data.value;
            this.textCount.y = 42 + (_loc4_ ? 0 : 5);
            this.textCount.textColor = _loc4_ ? (_data.category == 29 ? Colors.GOLD : Colors.REGULAR) : Colors.SILVER;
         }
      }
   }
}

