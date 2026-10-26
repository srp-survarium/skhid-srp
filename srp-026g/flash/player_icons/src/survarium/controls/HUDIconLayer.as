package survarium.controls
{
   import flash.display.MovieClip;
   import flash.filters.DropShadowFilter;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;
   import flash.utils.getDefinitionByName;
   
   public class HUDIconLayer extends MovieClip
   {
      
      protected static const to_dir:Number = 180 / Math.PI;

      // The game's icon ids: 0 to 36 its players' (player::initialize_player_icon 0x1F29D0, the player's
      // index), from 37 the match's objects' (game_world_ui::initialize_match 0x2357A9.., update_player_icons
      // 0x23AA83: the batteries and the like).
      public static const FIRST_OBJECT:uint = 37;

      // The match's objects' icons hidden (the interface forced hidden: MainTimeline.force_interface_hidden).
      protected var _objectsHidden:Boolean = false;
      
      protected var _markers:Dictionary = new Dictionary();
      
      protected var _timeInterval:uint = 500;
      
      protected var _class:Dictionary = new Dictionary();
      
      protected var _enemyIds:Array = [];
      
      protected var _playerTeam:uint = 0;

      // Research's units on the screen (research_markers).
      protected var _research:Array = [];

      protected var _researchFormat:TextFormat;

      public function HUDIconLayer()
      {
         super();
      }

      // Research (mode 5), at a change and every second (research_native_026g.c
      // markers): the game's own team, the number of units, then ten numbers a
      // unit: cp_id, whether it is on the screen (off it or behind the camera:
      // hidden, as 0.27d), x, y in pixels, its distance from the camera in
      // metres, owner, the team taking it, its capture 0..100, the ms left of
      // its capture or neutralisation (0 none), whether the game's own player
      // is inside. Under the players' icons.
      public function research_markers(values:Array) : void
      {
         var ownTeam:uint = uint(values[0]);
         var count:uint = uint(values[1]);
         var i:uint = 0;
         var at:uint;
         var marker:ResearchMarker;
         var probe:TextField;
         if(this._researchFormat == null && count > 0)
         {
            // the markers' labels' font as the movie embeds it (MarkerLabel:
            // Open Sans Condensed bold, the one bold face it has; the
            // players' names are Open Sans regular), its face kept
            probe = new MarkerLabel().textField;
            this._researchFormat = probe.length > 0 ? probe.getTextFormat() : probe.defaultTextFormat;
            this._researchFormat.size = 15;
            this._researchFormat.color = 0xFFFFFF;
         }
         while(this._research.length < count)
         {
            marker = new ResearchMarker(this._researchFormat);
            marker.visible = false;
            addChildAt(marker,0);
            this._research.push(marker);
         }
         while(i < this._research.length)
         {
            marker = this._research[i];
            at = 2 + 10 * i;
            if(i >= count || at + 9 >= values.length || uint(values[at + 1]) == 0)
            {
               marker.visible = false;
            }
            else
            {
               marker.x = Number(values[at + 2]);
               marker.y = Number(values[at + 3]);
               marker.update(uint(values[at]),uint(values[at + 5]),uint(values[at + 6]),Number(values[at + 7]),Number(values[at + 8]),uint(values[at + 9]) != 0,ownTeam,Number(values[at + 4]));
               marker.visible = true;
            }
            i++;
         }
      }
      
      public function set objectsHidden(hidden:Boolean) : void
      {
         var id:*;
         _objectsHidden = hidden;
         if(hidden)
         {
            for(id in this._markers)
            {
               if(uint(id) >= FIRST_OBJECT)
               {
                  this._markers[id].visible = false;
               }
            }
         }
      }

      // Whether the icon of this id may show: an enemy's player icon never (the game's own rule), an object's
      // not while the objects are hidden.
      protected function canShow(id:uint, renderer:HUDMarker) : Boolean
      {
         if(_objectsHidden && id >= FIRST_OBJECT)
         {
            return false;
         }
         return renderer.kind != 0 || this._enemyIds.indexOf(id) == -1;
      }

      public function register_icon_renderer(kind:uint, renderer_class:Class) : void
      {
         this._class[kind] = renderer_class;
      }
      
      public function create_player_icon(id:uint, kind:uint, baseScale:Number) : void
      {
         var renderer:HUDMarker = new this._class[kind]() as HUDMarker;
         renderer.baseScale = baseScale;
         if(kind == 0)
         {
            (renderer.textField as TextField).autoSize = TextFieldAutoSize.CENTER;
            renderer.textField.visible = false;
         }
         renderer.kind = kind;
         this._markers[id] = renderer;
         renderer.visible = false;
         addChild(renderer);
      }
      
      public function set_enemy_icon_ids(value:Array) : void
      {
         trace("set_player_icon_ids::",value,"length::",value.length);
         _enemyIds = value;
      }
      
      public function set_player_team(team:uint) : void
      {
         _playerTeam = team;
      }
      
      public function set_player_icon_team(id:uint, team:uint) : void
      {
         this._markers[id].team = team;
      }
      
      public function set_player_icon_name(id:uint, str:String) : void
      {
         this._markers[id].textField.text = str;
      }
      
      public function set_player_icon_type(id:uint, type:uint) : void
      {
         this._markers[id].type = type;
      }
      
      public function set_player_icon_container(id:uint, container:Boolean) : void
      {
         this._markers[id].container = container;
      }
      
      public function set_player_icon_rotation(id:uint, rot:Number) : void
      {
         this._markers[id].rotation = rot * to_dir;
      }
      
      public function set_player_icon_label(id:uint, label:String) : void
      {
         this._markers[id].label = label;
      }
      
      public function set_player_icon_glow(id:uint, bool:Boolean) : void
      {
         this._markers[id].glow = bool;
      }
      
      public function set_player_time_interval(value:uint) : void
      {
         var _marker:HUDMarker;
         for each(_marker in this._markers)
         {
            _marker.delay = value;
         }
      }
      
      public function set_player_icon_pos(id:uint, posX:Number, posY:Number, scale:Number) : void
      {
         var renderer:HUDMarker = this._markers[id];
         if(!renderer)
         {
            return;
         }
         renderer.x = posX;
         renderer.y = posY;
         renderer.scaleX = renderer.scaleY = scale * renderer.baseScale;
      }
      
      public function set_player_icon_visible(id:uint, bool:Boolean) : void
      {
         var canBeVisible:Boolean;
         if(this._markers[id])
         {
            if(bool)
            {
               trace("set_player_icon_visible:: kind:",this._markers[id].kind,"indexInEnemys:",this._enemyIds.indexOf(id));
               canBeVisible = this.canShow(id,this._markers[id]);
               if(canBeVisible)
               {
                  this._markers[id].visible = true;
               }
            }
            else
            {
               this._markers[id].visible = false;
            }
         }
      }
      
      public function set_player_icon_props(props:Object) : void
      {
         var id:int = (props.id);
         var renderer:HUDMarker = this._markers[id];
         trace("set_player_icon_props",id,renderer);
         if(!renderer)
         {
            return;
         }
         var iconVisible:Boolean = props.visible == true || props.visible == undefined;
         var canBeVisible:Boolean;
         var textField:TextField;
         var newName:String;
         var colorTransfor:ColorTransform;
         var color:uint;
         var filter:DropShadowFilter;
         if(iconVisible)
         {
            canBeVisible = this.canShow(id,renderer);
            trace("canBeVisible",canBeVisible,renderer.kind,props.a);
            if(canBeVisible)
            {
            if(props.visible != undefined)
            {
               renderer.visible = props.visible;
            }
            if(props.x != undefined)
            {
               renderer.x = props.x;
               renderer.y = props.y;
               renderer.scaleX = renderer.scaleY = props.z * renderer.baseScale;
            }
            }
            else
            {
               return;
            }
            textField = renderer.textField;
            trace("textField",textField);
            if(textField)
            {
               if(props.name != undefined)
               {
                  newName = props.name;
                  if(textField.text != newName)
                  {
                     textField.text = newName;
                  }
               }
               if(props.modifier_visible != undefined)
               {
                  renderer.container = props.modifier_visible;
               }
               if(props.name_visible != undefined)
               {
                  if(props.name_visible)
                  {
                     renderer.showName();
                  }
                  else
                  {
                     renderer.hideName();
                  }
               }
            }
            if(props.rgb != undefined)
            {
               colorTransfor = new ColorTransform();
               color = (props.rgb);
               colorTransfor.color = color;
               if(renderer.icon.renderer)
               {
                  renderer.icon.renderer.transform.colorTransform = colorTransfor;
               }
               if(textField)
               {
                  textField.textColor = color;
                  if(textField.filters)
                  {
                     filter = textField.filters[0];
                     filter.color = color;
                  }
               }
            }
            if(props.a != undefined)
            {
               renderer.alpha = props.a / 255;
            }
         }
         else
         {
            renderer.visible = false;
         }
      }
      
      public function set_player_name_visible(id:uint, bool:Boolean) : void
      {
         var mk:HUDMarker = this._markers[id];
         if(!mk)
         {
            return;
         }
         if(bool)
         {
            mk.showName();
         }
         else
         {
            mk.hideName();
         }
      }
      
      public function set_player_icon_alpha(id:uint, alpha:uint) : void
      {
         var renderer:HUDMarker = this._markers[id];
         if(renderer)
         {
            renderer.alpha = alpha / 255;
         }
      }
      
      public function set_player_color(id:uint, red:uint, green:uint, blue:uint, alpha:uint = 255) : void
      {
         var color:uint = (red << 16 | green << 8 | blue);
         var colorTransfor:ColorTransform = new ColorTransform();
         colorTransfor.color = color;
         colorTransfor.alphaOffset = alpha - 255;
         var renderer:HUDMarker = this._markers[id];
         var filter:DropShadowFilter;
         if(renderer.icon.renderer)
         {
            renderer.icon.renderer.transform.colorTransform = colorTransfor;
         }
         if(renderer.textField)
         {
            renderer.textField.textColor = color;
         }
         if(renderer.textField && ((renderer.textField as TextField).filters))
         {
            filter = (renderer.textField as TextField).filters[0];
            filter.color = red << 16 | green << 8 | blue;
         }
      }
   }
}

