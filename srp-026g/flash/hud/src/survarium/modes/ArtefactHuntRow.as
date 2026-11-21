package survarium.modes
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.text.TextField;
   import survarium.controls.ResearchUnits;
   import survarium.controls.TopRow;
   import survarium.data.LocalizationData;

   // Artifact Hunt (mode 13): the artefact at a glance, under the teams' bars where Research has its
   // units, in their look (ArtefactHuntLook). One circle, the orb in it:
   // - none in the world (the wait for it): grey, the orb dim;
   // - lying: grey, the orb bright, a ring as far round as its life left, the time under it;
   // - carried: in the carrier's team's colour, his name under it; the game's own player's (or the
   //   watched one's) larger, "You carry the artifact" under it.
   // Under that, while sudden death is on or comes with the next round, "Sudden death"; then the
   // round's own countdowns (0.46c2's "Round restarts in" and its pre-round text): the 3 s after a
   // delivery that a new round follows, and the round's 5 s at the spawns. Drawn from the last
   // update (draw) and the time since it came (tick, every frame): nothing kept of earlier ones.
   public class ArtefactHuntRow extends TopRow
   {
      protected static const RADIUS:Number = 14;

      protected static const RING:Number = 4;

      // The game's own carry: larger, as Research's unit the player is inside.
      public static const OWN_CARRY:Number = ResearchUnits.INSIDE;

      // From the bottom of the bars to the circle's centre (HUD.placeTopRow), room for OWN_CARRY.
      public static const TOP:Number = Math.ceil((RADIUS + RING) * OWN_CARRY) + 4;

      protected static const GAP:Number = 2;

      protected var _pip:Shape = new Shape();

      protected var _caption:TextField;

      protected var _sudden:TextField;

      protected var _banner:TextField;

      protected var _state:ArtefactHuntState;

      protected var _carrierName:String = "";

      // `captionFont`, `bannerFont`: labels of the movie whose embedded fonts the texts take
      // (Research's timers' and the pre-match countdown's).
      public function ArtefactHuntRow(captionFont:TextField, bannerFont:TextField)
      {
         super();
         addChild(this._pip);
         this._caption = ArtefactHuntLook.text(captionFont,16);
         this._sudden = ArtefactHuntLook.text(captionFont,16,ArtefactHuntLook.SUDDEN);
         this._banner = ArtefactHuntLook.text(bannerFont,18);
         addChild(this._sudden);
         addChild(this._caption);
         addChild(this._banner);
         visible = false;
      }

      // The state and the carrier's name (the Tab list's row of his place; "" unknown).
      public function draw(state:ArtefactHuntState, carrierName:String, now:int) : void
      {
         this._state = state;
         this._carrierName = carrierName == null ? "" : carrierName;
         this.tick(now);
      }

      public function clear() : void
      {
         this._state = null;
         visible = false;
      }

      public function tick(now:int) : void
      {
         var s:ArtefactHuntState = this._state;
         visible = s != null && s.phase != ArtefactHuntState.BEFORE_START;
         if(!visible)
         {
            return;
         }
         this.drawPip(s,now);
         var y:Number = (RADIUS + RING) * this._pip.scaleY + GAP;
         y = this.line(this._sudden,s.sudden != ArtefactHuntState.SUDDEN_OFF ?
            LocalizationData.getName("st_srp_hud_artefact_hunt_sudden_death") : "",y);
         y = this.line(this._caption,this.caption(s,now),y);
         this._caption.textColor = this.captionColour(s);
         this.line(this._banner,this.banner(s,now),y + 2);
      }

      protected function drawPip(s:ArtefactHuntState, now:int) : void
      {
         var g:Graphics = this._pip.graphics;
         var held:Boolean = s.artefact == ArtefactHuntState.HELD;
         g.clear();
         this._pip.scaleX = this._pip.scaleY = s.viewerCarries ? OWN_CARRY : 1;
         g.beginFill(0,0.45);
         g.drawCircle(0,0,RADIUS + RING);
         g.endFill();
         g.beginFill(held ? ArtefactHuntLook.teamColour(s.carrierTeam,s) : ArtefactHuntLook.NOBODY,
            s.artefact == ArtefactHuntState.NOWHERE ? 0.5 : 0.9);
         g.drawCircle(0,0,RADIUS);
         g.endFill();
         if(s.artefact == ArtefactHuntState.LYING && s.phaseWhole > 0)
         {
            ResearchUnits.drawRing(g,RADIUS,RING,s.phaseLeftAt(now) / s.phaseWhole,ArtefactHuntLook.GLOW);
         }
         ArtefactHuntLook.drawOrb(g,RADIUS * 0.62,s.artefact != ArtefactHuntState.NOWHERE);
      }

      protected function caption(s:ArtefactHuntState, now:int) : String
      {
         if(s.viewerCarries)
         {
            return LocalizationData.getName("st_srp_hud_artefact_hunt_carrying");
         }
         if(s.artefact == ArtefactHuntState.HELD)
         {
            return this._carrierName;
         }
         if(s.artefact == ArtefactHuntState.LYING && s.phaseWhole > 0)
         {
            return ResearchUnits.timeText(s.phaseLeftAt(now));
         }
         return "";
      }

      protected function captionColour(s:ArtefactHuntState) : uint
      {
         if(s.viewerCarries)
         {
            return ArtefactHuntLook.GLOW;
         }
         return s.artefact == ArtefactHuntState.HELD ? ArtefactHuntLook.teamColour(s.carrierTeam,s) : 0xFFFFFF;
      }

      // 0.46c2's round texts: "Round restarts in" with the 3 s left when a round follows, the
      // pre-match text with the round's 5 s at the spawns.
      protected function banner(s:ArtefactHuntState, now:int) : String
      {
         if(s.restarting)
         {
            return LocalizationData.getName("st_round_restart_in") + " " + ArtefactHuntLook.clock(s.phaseLeftAt(now));
         }
         if(s.frozen)
         {
            return LocalizationData.getName("st_final_countdown") + " " + ArtefactHuntLook.clock(s.phaseLeftAt(now));
         }
         return "";
      }

      // A text under the last, centred; "" hides it. The y under it.
      protected function line(field:TextField, text:String, y:Number) : Number
      {
         field.visible = text.length > 0;
         if(field.text != text)
         {
            field.text = text;
         }
         if(!field.visible)
         {
            return y;
         }
         field.x = Math.round(-field.width / 2);
         field.y = Math.round(y);
         return field.y + field.height;
      }

      override public function get contentBottom() : Number
      {
         var bottom:Number = (RADIUS + RING) * this._pip.scaleY;
         for each(var field:TextField in [this._sudden,this._caption,this._banner])
         {
            if(field.visible)
            {
               bottom = Math.max(bottom,field.y + field.height);
            }
         }
         return bottom;
      }

      // The texts shown now, for checks.
      public function get texts() : Object
      {
         return {sudden:this._sudden.visible ? this._sudden.text : "",caption:this._caption.visible ? this._caption.text : "",
            banner:this._banner.visible ? this._banner.text : ""};
      }

      public function get carryScale() : Number
      {
         return this._pip.scaleY;
      }
   }
}
