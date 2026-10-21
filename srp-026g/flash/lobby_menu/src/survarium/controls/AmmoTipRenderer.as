package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.TipRenderer;
   import survarium.data.LocalizationData;
   
   public class AmmoTipRenderer extends TipRenderer
   {
      
      public var background:Sprite;
      
      public var lockSprite:Sprite;
      
      public var moneyIcon:MovieClip;
      
      public var textField:TextField;
      
      public var textFieldCost:TextField;
      
      public var textFieldCostLabel:TextField;
      
      public var textFieldLock:TextField;
      
      public var tiledSprite:TiledSprite;
      
      public function AmmoTipRenderer()
      {
         super();
      }
      
      override protected function invalidateData() : void
      {
         var _loc1_:Object = _data as Object;
         trace("AmmoTipRenderer::",_loc1_,_data);
         if(_loc1_)
         {
            trace("_data",_data.cost);
            this.lockSprite.visible = _loc1_.lock;
            this.textFieldLock.visible = _loc1_.lock;
            this.textFieldLock.text = _loc1_.lockText;
            this.textField.y = this.textFieldLock.visible ? 28 : 3;
            this.textField.width = 144;
            this.textFieldLock.width = 131;
            this.textField.styleSheet = MainScene.textStyleSheet;
            this.textField.text = _loc1_.tipText;
            this.textField.autoSize = TextFieldAutoSize.LEFT;
            this.textFieldCostLabel.y = this.textField.y + this.textField.textHeight + 10;
            this.textFieldCost.y = this.textFieldCostLabel.y;
            this.moneyIcon.y = this.textFieldCostLabel.y + 6;
            this.textFieldCostLabel.text = LocalizationData.getName("cost") + ": ";
            this.moneyIcon.x = this.textFieldCostLabel.textWidth + this.textFieldCostLabel.x + 5;
            this.moneyIcon.gotoAndStop(_loc1_.isPremium == true ? 2 : 1);
            this.textFieldCost.text = _loc1_.cost.toString();
            this.textFieldCost.x = this.moneyIcon.x + this.moneyIcon.width + 3;
            this.background.width = Math.max(this.textFieldCost.x + this.textFieldCost.textWidth + 15,this.textField.x + this.textField.textWidth + 15);
            this.background.height = this.textFieldCost.y + 30;
         }
         super.invalidateData();
      }
   }
}

