package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.utils.Constraints;
   
   public class mc_text_warning extends UIComponent
   {
      
      public var contextText:TextField;
      
      public var icon:MovieClip;
      
      public function mc_text_warning()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         super.configUI();
         constraints.addElement("contextText",this.contextText,Constraints.LEFT | Constraints.RIGHT);
         constraints.addElement("icon",this.icon,Constraints.CENTER_H);
      }
      
      public function setText(value:String) : void
      {
         var border:int = 40;
         this.contextText.text = value;
         var newWidth:int = this.contextText.textWidth + border;
         scaleX = newWidth / 265;
         setSize(newWidth,height);
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SIZE))
         {
            constraints.update(_width,_height);
         }
      }
   }
}

