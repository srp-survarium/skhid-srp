package survarium.item
{
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ToolTip;
   import scaleform.gfx.TextFieldEx;
   import survarium.Util;
   import survarium.constants.Category;
   import survarium.constants.Colors;
   import survarium.controls.IconRenderer;
   import survarium.core.Icon;
   
   public class ItemRendererReward extends ToolTip
   {
      
      private var _textFieldFormat:TextFormat;
      
      public var iconRenderer:IconRenderer;
      
      public var textField:TextField;
      
      public function ItemRendererReward()
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
         _textFieldFormat = this.textField.getTextFormat();
         TextFieldEx.setVerticalAlign(this.textField,TextFieldEx.VALIGN_CENTER);
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
               this.iconRenderer.content = null;
            }
         }
      }
      
      protected function updateData() : void
      {
         var _loc1_:Boolean;
         var _loc2_:Boolean;
         this.iconRenderer.content = _data.icon ? (_data.icon as Icon).clone() : null;
         if(!_data.isStack)
         {
            this._textFieldFormat.align = TextFormatAlign.CENTER;
            this.textField.defaultTextFormat = this._textFieldFormat;
            this.textField.text = _data.name;
            this.textField.textColor = Colors.REGULAR;
         }
         else
         {
            _loc1_ = _data.category == Category.GOLD || _data.category == Category.REGULAR;
            _loc2_ = _data.superCategory == Category.BASE_VOUCHER;
            this._textFieldFormat.align = _loc2_ ? TextFormatAlign.CENTER : TextFormatAlign.RIGHT;
            this.textField.defaultTextFormat = this._textFieldFormat;
            if(_loc1_)
            {
               this.textField.text = Util.getMoneyString(_data.value);
            }
            else
            {
               this.textField.text = _loc2_ ? Util.getMoneyString(_data.value) + " " + String(_data.name) : "x" + Util.getMoneyString(_data.value);
            }
            this.textField.textColor = _loc2_ ? (_data.category == Category.GOLD ? Colors.GOLD : Colors.REGULAR) : Colors.SILVER;
         }
      }
   }
}

