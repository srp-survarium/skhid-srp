package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.controls.TipRenderer;
   import scaleform.clik.utils.ConstrainedElement;
   import scaleform.clik.utils.Constraints;
   import survarium.core.Icon;
   
   public class ProfileTipRenderer extends TipRenderer
   {
      
      public var textField:TextField;
      
      public var spriteAnchor:MovieClip;
      
      public var constraints:Constraints;
      
      protected var _icon:Icon;
      
      public function ProfileTipRenderer()
      {
         super();
         this._icon = new Icon();
         this.spriteAnchor.addChild(this._icon);
         this.constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         this.constraints.addElement("textField",this.textField,Constraints.ALL);
         this.constraints.addElement("spriteAnchor",this.spriteAnchor,Constraints.LEFT);
      }
      
      override protected function invalidateData() : void
      {
         this.textField.text = _data.name;
         if(_data.icon is Icon)
         {
            this._icon.applyImage(_data.icon);
         }
         width = this.textField.textWidth;
         var textElement:ConstrainedElement = this.constraints.getElement("textField");
         var iconElement:ConstrainedElement = this.constraints.getElement("spriteAnchor");
         var newWidth:Number = Math.ceil(this.textField.textWidth + textElement.left + textElement.right + 5 + this.spriteAnchor.width);
         var oldWidth:Number = width;
         width = newWidth;
         this.constraints.update(width,height);
      }
   }
}

