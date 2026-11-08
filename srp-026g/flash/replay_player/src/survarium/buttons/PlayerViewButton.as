package survarium.buttons
{
   import flash.text.TextFormat;
   import scaleform.clik.controls.Button;
   import survarium.constants.Colors;
   import survarium.constants.TeamSide;
   
   public class PlayerViewButton extends Button
   {
      
      public var btnFirstView:Button;
      
      public var btnThirdView:Button;

      // srp's: the smallest size a long name is shrunk to, to fit its field whole
      private static const MIN_NAME_SIZE:Number = 8;

      // the name's size in the art (0: not read yet)
      private var _nameSize:Number = 0;
      
      public function PlayerViewButton()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         mouseChildren = true;
      }
      
      // the name fitted again once the button is drawn (its field's size is settled then)
      override protected function draw() : void
      {
         super.draw();
         if(_data)
         {
            this.fitName(String(_data.name),_data.team == TeamSide.ENEMY ? Colors.ENEMY : Colors.OTHER_PLAYER);
         }
      }

      override protected function updateText() : void
      {
         if(_data)
         {
            this.fitName(String(_data.name),_data.team == TeamSide.ENEMY ? Colors.ENEMY : Colors.OTHER_PLAYER);
         }
      }

      // The text shown whole: no part scrolled out (the field's margins take room besides its width).
      protected function fits() : Boolean
      {
         return textField.maxScrollH == 0 && textField.textWidth <= textField.width - 4;
      }

      // srp's: the name whole in its colour, at the art's size, or smaller until it fits its field
      // (a 16-character name was cut at the art's size); one too wide even then ends with "…".
      protected function fitName(name:String, colour:uint) : void
      {
         var format:TextFormat = textField.defaultTextFormat;
         if(this._nameSize == 0)
         {
            this._nameSize = Number(format.size) > 0 ? Number(format.size) : 14;
         }
         var size:Number = this._nameSize;
         textField.text = name;
         while(true)
         {
            format.size = size;
            format.color = colour;
            textField.setTextFormat(format);
            if(this.fits() || size <= MIN_NAME_SIZE)
            {
               break;
            }
            size -= 1;
         }
         for(var cut:int = name.length - 1; cut > 0 && !this.fits(); cut--)
         {
            textField.text = name.substr(0,cut) + "…";
            textField.setTextFormat(format);
         }
      }
   }
}

