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
         var condition:int;
         var trackColor:uint;
         var colorTransform:ColorTransform;
         var isMoney:Boolean;
         this.iconRenderer.content = _data.icon;
         this.damageIndicator.visible = !_data.isStack && _data.value < 100;
         trace("IR::updateData",_data.name,_data.isStack,_data.value,this.damageIndicator.visible);
         this.iconRenderer.y = _data.category == 26 ? -5 : -10;
         if(!_data.isStack)
         {
            this.textCount.visible = false;
            this.damageIndicator.track.width = 165 * _data.value / 100;
            condition = _data.value / 100 * 510;
            trackColor = Util.getColor(Math.min(510 - condition,255),Math.min(condition,255),0);
            colorTransform = new ColorTransform();
            colorTransform.color = trackColor;
            this.damageIndicator.track.transform.colorTransform = colorTransform;
         }
         else
         {
            this.textCount.visible = true;
            this._countTextFormat.align = _data.superCategory == 5 ? TextFormatAlign.CENTER : TextFormatAlign.RIGHT;
            this.textCount.defaultTextFormat = this._countTextFormat;
            isMoney = _data.superCategory == 5;
            this.textCount.text = (isMoney ? "" : "x") + _data.value;
            this.textCount.y = 42 + (isMoney ? 0 : 5);
            this.textCount.textColor = isMoney ? (_data.category == 29 ? Colors.GOLD : Colors.REGULAR) : Colors.SILVER;
         }
      }
   }
}

